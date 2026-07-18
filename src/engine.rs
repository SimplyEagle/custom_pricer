use sqlx::PgPool;
use tracing::{info, warn, debug};
use crate::db;
use crate::traits;
use crate::schema::SchemaMap;

#[derive(Debug)]
pub struct MarketData {
    pub buy_metal: f32,
    pub sell_metal: f32,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidityTier {
    High, // Core currencies, tickets, vouchers (lookback: 1-7 days)
    Mid,  // Standard Stranges, standard Killstreaks/Australiums (lookback: 30 days)
    Low,  // Unusuals, Warpaints, Australium Pro KS combos (lookback: 90-180 days)
}

#[derive(Debug, Clone)]
pub struct PriceConfig {
    pub tier: LiquidityTier,
    pub lookback_days: i32,
    pub min_volume: i64,
}

pub fn classify_sku(sku: &str) -> PriceConfig {
    let high_velocity_skus = ["5021;6", "725;6", "728;6"];

    let is_high_velocity = high_velocity_skus.iter().any(|&s| sku.starts_with(s));
    let is_unusual = sku.split(';').nth(1) == Some("5");
    let is_warpaint = sku.contains(";pk");
    let is_australium = sku.contains(";australium");
    let is_pro_ks = sku.contains(";kt-3");

    if is_high_velocity {
        PriceConfig {
            tier: LiquidityTier::High,
            lookback_days: 7,
            min_volume: 1, // Lowered to 1 to prevent API blocking fresh DBs
        }
    } else if is_unusual || is_warpaint || (is_australium && is_pro_ks) {
        PriceConfig {
            tier: LiquidityTier::Low,
            lookback_days: 180,
            min_volume: 1,
        }
    } else {
        PriceConfig {
            tier: LiquidityTier::Mid,
            lookback_days: 30,
            min_volume: 1,
        }
    }
}

pub async fn calculate_market_price(sku: &str, pool: &PgPool, live_key_price: f32, schema: &SchemaMap) -> Option<MarketData> {
    let config = classify_sku(sku);

    // 1. Direct Database Match (The Market Spread)
    if let Some(spread) = db::get_adaptive_median(sku, config.lookback_days, config.min_volume, pool).await {
        return Some(MarketData {
            buy_metal: spread.buy_metal,
            sell_metal: spread.sell_metal,
            source: "Database (Exact Match)".to_string(),
        });
    }

    // 2. Trait Deconstruction Fallback
    if config.tier == LiquidityTier::Low || sku.contains(";kt-") || sku.contains(";sp-") {
        if let Some(spread) = deconstruct_and_price(sku, pool).await {
            return Some(MarketData {
                buy_metal: spread.buy_metal,
                sell_metal: spread.sell_metal,
                source: "Database (Trait Deconstruction)".to_string(),
            });
        }
    }

    // 3. SCM Fallback
    if let Some(scm_price) = crate::scm::fetch_fallback_price(sku, live_key_price, schema).await {
        return Some(MarketData {
            buy_metal: scm_price * 0.90, // Apply a safe 10% buy margin for Steam fiat conversions
            sell_metal: scm_price * 1.05, 
            source: "API (Steam Community Market)".to_string(),
        });
    }

    warn!("⚠️ [Engine] No database data or trait matches found for {}. Relaying to fallbacks.", sku);
    None
}

async fn deconstruct_and_price(sku: &str, pool: &PgPool) -> Option<MarketData> {
    let mut total_buy = 0.0;
    let mut total_sell = 0.0;

    let (clean_sku, strange_parts) = extract_strange_parts(sku);
    let base_sku = strip_all_premium_modifiers(&clean_sku);

    // --- STEP A: Price the Base Asset ---
    let base_spread = if base_sku == clean_sku {
        db::get_adaptive_median(&base_sku, 30, 1, pool).await?
    } else {
        db::get_adaptive_median(&base_sku, 30, 1, pool).await.unwrap_or_else(|| {
            debug!("⚠️ [Engine] Base SKU {} missing, defaulting to generic weapon.", base_sku);
            db::MarketSpread { buy_metal: 0.50, sell_metal: 0.55 }
        })
    };

    total_buy += base_spread.buy_metal;
    total_sell += base_spread.sell_metal;

    // --- STEP B: Professional/Specialized Killstreak Premiums ---
    if sku.contains(";kt-") {
        let is_pro = sku.contains(";kt-3");
        let sheen_id = extract_modifier_id(sku, ";sheen-");
        let streaker_id = extract_modifier_id(sku, ";streaker-");

        let generic_kit_value = if is_pro { 350.0 } else { 45.0 }; 
        let mut mult = 1.0;

        if let Some(s_id) = sheen_id {
            mult *= traits::Sheen::from_id(s_id as i32).market_multiplier();
        }
        if is_pro {
            if let Some(str_id) = streaker_id {
                mult *= traits::Killstreaker::from_id(str_id as i32).market_multiplier();
            }
        }

        let ks_premium = generic_kit_value * mult;
        total_buy += ks_premium;
        total_sell += ks_premium;
        debug!("⚔️ [Engine] Appended Killstreak Premium: +{:.2} ref (Multiplier: {:.2}x)", ks_premium, mult);
    }

    // --- STEP C: Unusual Particle Premiums ---
    if sku.contains(";u") {
        if let Some(effect_id) = extract_modifier_id(sku, ";u") {
            let effect_sku = format!("unusual_effect_{}", effect_id);
            let effect_spread = db::get_adaptive_median(&effect_sku, 180, 1, pool)
                .await
                .unwrap_or(db::MarketSpread { buy_metal: 700.0, sell_metal: 800.0 });

            let is_cancer = traits::is_cancer_hat(&base_sku);
            
            let unusual_buy = traits::calculate_unusual_premium(base_spread.buy_metal, effect_spread.buy_metal, is_cancer);
            let unusual_sell = traits::calculate_unusual_premium(base_spread.sell_metal, effect_spread.sell_metal, is_cancer);

            total_buy += unusual_buy;
            total_sell += unusual_sell;
            debug!("✨ [Engine] Appended Unusual Effect Premium: Buy +{:.2} / Sell +{:.2}", unusual_buy, unusual_sell);
        }
    }

    // --- STEP D: Strange Part 20% Applied Rule ---
    for part_defindex in strange_parts {
        let part_sku = format!("{};6", part_defindex);
        let part_spread = db::get_adaptive_median(&part_sku, 90, 1, pool)
            .await
            .unwrap_or(db::MarketSpread { buy_metal: 85.0, sell_metal: 95.0 });

        let part_buy = traits::calculate_strange_parts_premium(part_spread.buy_metal);
        let part_sell = traits::calculate_strange_parts_premium(part_spread.sell_metal);

        total_buy += part_buy;
        total_sell += part_sell;
        debug!("🎯 [Engine] Appended Strange Part ({}): Buy +{:.2} / Sell +{:.2}", part_defindex, part_buy, part_sell);
    }

    info!("⚙️ [Engine] Reconstructed complex SKU {}. Buy: {:.2} ref | Sell: {:.2} ref", sku, total_buy, total_sell);
    
    Some(MarketData {
        buy_metal: total_buy,
        sell_metal: total_sell,
        source: "Database (Trait Deconstruction)".to_string(),
    })
}

// --- HELPER PARSING FUNCTIONS ---
fn extract_strange_parts(sku: &str) -> (String, Vec<i64>) {
    let mut parts = Vec::new();
    let mut clean_segments = Vec::new();
    for segment in sku.split(';') {
        if segment.starts_with("sp-") {
            if let Ok(defindex) = segment[3..].parse::<i64>() {
                parts.push(defindex);
            }
        } else {
            clean_segments.push(segment);
        }
    }
    (clean_segments.join(";"), parts)
}

fn strip_all_premium_modifiers(sku: &str) -> String {
    sku.split(';')
        .filter(|seg| {
            !seg.starts_with("kt-")
                && !seg.starts_with("sheen-")
                && !seg.starts_with("streaker-")
                && !seg.starts_with("u")
        })
        .collect::<Vec<&str>>()
        .join(";")
}

fn extract_modifier_id(sku: &str, pattern: &str) -> Option<i64> {
    sku.split(';')
        .find(|seg| seg.starts_with(pattern.trim_start_matches(';')))
        .and_then(|seg| {
            let val_str = seg.trim_start_matches(pattern.trim_start_matches(';'));
            val_str.parse::<i64>().ok()
        })
}