use crate::state::AppState;
use crate::db;
use crate::traits;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum LiquidityTier {
    High,   // Keys, Metal, Tickets
    Medium, // Standard Stranges, Popular Cosmetics
    Low,    // Unusuals, Pro KS
}

pub struct LiquidityProfile {
    pub tier: LiquidityTier,
    pub lookback_days: i32,
    pub min_volume: i64,
}

impl LiquidityProfile {
    /// Determines the adaptive strategy based on the SKU format
    pub fn from_sku(sku: &str) -> Self {
        if sku == "5021;6" || sku == "725;6" { 
            // Highly Liquid Items
            LiquidityProfile { tier: LiquidityTier::High, lookback_days: 7, min_volume: 50 }
        } else if sku.contains(";u") || sku.contains("ks-3") { 
            // Unusuals & Professional Killstreaks
            LiquidityProfile { tier: LiquidityTier::Low, lookback_days: 90, min_volume: 2 }
        } else {
            // Standard Items
            LiquidityProfile { tier: LiquidityTier::Medium, lookback_days: 30, min_volume: 10 }
        }
    }
}

/// The core intelligence engine. Calculates the raw metal value of an item.
pub async fn calculate_item_value(sku: &str, state: Arc<AppState>) -> Result<f32, String> {
    let profile = LiquidityProfile::from_sku(sku);
    
    // Tier 1: Try the adaptive database rolling median
    if let Some(median) = db::get_adaptive_median(sku, profile.lookback_days, profile.min_volume, &state.db_pool).await {
        println!("✅ [Engine] Local median found for {} using {}-day window.", sku, profile.lookback_days);
        return Ok(median);
    }

    // Tier 2 & 3: Fallbacks depending on item type
    match profile.tier {
        LiquidityTier::High | LiquidityTier::Medium => {
            println!("🔄 [Engine] Falling back to pricedb.io for liquid item: {}", sku);
            // TODO: Implement reqwest call to pricedb.io/api/items
            Ok(25.33) // Mock fallback value
        }
        LiquidityTier::Low => {
            println!("🛠️ [Engine] Low liquidity detected. Triggering component trait pricing for: {}", sku);
            
            // Route to the correct trait deconstruction algorithm
            if sku.contains("ks-3") {
                
                // TODO: Fetch the generic kit median value from DB/API. Mocking at 100 ref.
                let generic_kit_metal_value = 100.0;
                
                let calculated_value = traits::calculate_pro_ks_premium(sku, generic_kit_metal_value);
                println!("✨ [Engine] Pro KS Premium Calculated: {} ref", calculated_value);
                Ok(calculated_value)

            } else if sku.contains(";u") {
                
                // TODO: Fetch base hat value and effect median from DB.
                let base_hat_value_metal = 20.0; // Mock: standard craft hat base value
                let effect_median_metal_value = 500.0; // Mock: 500 ref effect median
                let is_cancer_hat = false; // Mock: Assume it's a decent hat for now

                let calculated_value = traits::calculate_unusual_premium(
                    base_hat_value_metal,
                    effect_median_metal_value,
                    is_cancer_hat
                );
                println!("✨ [Engine] Unusual Premium Calculated: {} ref", calculated_value);
                Ok(calculated_value)

            } else {
                // Catch-all for other low liquidity items that aren't Unusuals or Pro KS
                Ok(250.00) 
            }
        }
    }
}