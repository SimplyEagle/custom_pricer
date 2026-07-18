use sqlx::PgPool;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy)]
pub struct MarketSpread {
    pub buy_metal: f32,
    pub sell_metal: f32,
}

/// Fetches medians across multiple timeframes inside a single query, running Shock Detection on the result
pub async fn get_adaptive_median(sku: &str, lookback_days: i32, min_volume: i64, pool: &PgPool) -> Option<MarketSpread> {
    let rows = match sqlx::query(
        r#"
        WITH stitched_listings AS (
            SELECT 
                sku, 
                intent, 
                price_total_metal, 
                created_at,
                1 as weight
            FROM historical_listings
            WHERE sku = $1 AND created_at >= NOW() - ($2::int * INTERVAL '1 day')
            
            UNION ALL
            
            SELECT 
                sku, 
                intent, 
                median_price as price_total_metal, 
                (record_date::text || ' 00:00:00')::timestamptz as created_at,
                volume as weight
            FROM historical_rollups
            WHERE sku = $1 AND record_date >= DATE(NOW() - ($2::int * INTERVAL '1 day'))
        ),
        weighted_listings AS (
            SELECT 
                intent, 
                price_total_metal,
                created_at
            FROM stitched_listings
            CROSS JOIN generate_series(1, GREATEST(1, weight))
        )
        SELECT 
            intent,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) FILTER (WHERE created_at >= NOW() - INTERVAL '1 day') as median_24h,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) FILTER (WHERE created_at >= NOW() - INTERVAL '7 days') as median_7d,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) as median_full,
            count(*) as total_volume
        FROM weighted_listings
        GROUP BY intent;
        "#
    )
    .bind(sku)
    .bind(lookback_days)
    .fetch_all(pool)
    .await {
        Ok(r) => r,
        Err(e) => {
            warn!("❌ [DB] Query error for SKU {}: {}", sku, e);
            return None;
        }
    };

    let mut buy_price: Option<f32> = None;
    let mut sell_price: Option<f32> = None;

    // Tight 4% leash for high velocity currency, 15% standard threshold for items
    let is_high_velocity = sku == "5021;6" || sku == "725;6" || sku == "728;6";
    let shock_threshold = if is_high_velocity { 0.04 } else { 0.15 };

    for row in rows {
        let intent: String = row.get("intent");
        let vol: i64 = row.get("total_volume");

        if vol < min_volume {
            warn!("⚠️ [DB] Insufficient volume for {} ({}) in lookback window. Found: {}/{}", sku, intent, vol, min_volume);
            continue;
        }

        let m_24h: Option<f64> = row.get("median_24h");
        let m_7d: Option<f64> = row.get("median_7d");
        let m_full: Option<f64> = row.get("median_full");

        let m_24h = m_24h.map(|v| v as f32);
        let m_7d = m_7d.map(|v| v as f32);
        let m_full = m_full.map(|v| v as f32);

        // Fallback cascades for timestamps that might have blank arrays
        let base_history = m_7d.or(m_full);
        
        let calculated_price = match (m_24h, base_history) {
            (Some(recent), Some(history)) => {
                if history > 0.0 {
                    let deviation = (recent - history).abs() / history;
                    if deviation > shock_threshold {
                        info!("🚨 [DB SHOCK DETECTED] SKU {} ({}) shifted by {:.1}%. Overriding to 24h recent median: {:.2} ref.",
                            sku, intent, deviation * 100.0, recent);
                        recent
                    } else {
                        // Standard market parameters: Blended weighted ratio
                        (history * 0.70) + (recent * 0.30)
                    }
                } else {
                    recent
                }
            },
            (Some(recent), None) => recent,
            (None, Some(history)) => history,
            (None, None) => continue,
        };

        if intent == "buy" {
            buy_price = Some(calculated_price);
        } else if intent == "sell" {
            sell_price = Some(calculated_price);
        }
    }

    match (buy_price, sell_price) {
        (Some(buy), Some(sell)) => Some(MarketSpread { buy_metal: buy, sell_metal: sell }),
        _ => None,
    }
}

/// Boot handler used specifically to fetch raw key statistics on startup
pub async fn get_24h_key_median(pool: &PgPool) -> Option<MarketSpread> {
    get_adaptive_median("5021;6", 1, 1, pool).await
}