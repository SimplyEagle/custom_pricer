use sqlx::{PgPool, Row};
use tracing::{debug, warn};

pub struct MarketSpread {
    pub buy_metal: f32,
    pub sell_metal: f32,
}

pub async fn get_adaptive_median(
    sku: &str,
    lookback_days: i32,
    min_volume: i64,
    pool: &PgPool,
) -> Option<MarketSpread> {
    
    let query = r#"
        SELECT 
            -- BUY Medians
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= 1 AND intent = 'buy') as buy_24h,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= 7 AND intent = 'buy') as buy_7d,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= $2::int AND intent = 'buy') as buy_long,
            
            -- SELL Medians
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= 1 AND intent = 'sell') as sell_24h,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= 7 AND intent = 'sell') as sell_7d,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= $2::int AND intent = 'sell') as sell_long,
            
            SUM(volume) as total_volume
        FROM (
            SELECT price_total_metal as price, intent, 1 as volume, EXTRACT(EPOCH FROM (NOW() - created_at))/86400 as age_days 
            FROM historical_listings 
            WHERE sku = $1 AND created_at >= NOW() - ($2::int * INTERVAL '1 day')
            
            UNION ALL
            
            SELECT median_price as price, intent, volume, EXTRACT(EPOCH FROM (NOW() - record_date))/86400 as age_days 
            FROM historical_rollups 
            WHERE sku = $1 AND record_date >= NOW() - ($2::int * INTERVAL '1 day')
        ) combined_data;
    "#;

    if let Ok(row) = sqlx::query(query).bind(sku).bind(lookback_days).fetch_one(pool).await {
        let volume: i64 = row.try_get("total_volume").unwrap_or(0);
        
        if volume >= min_volume {
            let b_long: f64 = row.try_get("buy_long").unwrap_or(0.0);
            let b_7d: f64 = row.try_get("buy_7d").unwrap_or(b_long);
            let b_24h: f64 = row.try_get("buy_24h").unwrap_or(b_7d);

            let s_long: f64 = row.try_get("sell_long").unwrap_or(0.0);
            let s_7d: f64 = row.try_get("sell_7d").unwrap_or(s_long);
            let s_24h: f64 = row.try_get("sell_24h").unwrap_or(s_7d);

            // Failsafe: Needs at least one valid dataset
            if b_long == 0.0 && s_long == 0.0 { return None; }

            // --- SHOCK DETECTION MATH ENGINE ---
            let shock_threshold = match sku {
                "5021;6" => 0.04, 
                "725;6" | "728;6" => 0.05, 
                _ => 0.15,
            };

            let mid_7d = (b_7d + s_7d) / 2.0;
            let mid_24h = (b_24h + s_24h) / 2.0;

            let shift_percentage = if mid_7d > 0.0 {
                (mid_24h - mid_7d).abs() / mid_7d
            } else { 0.0 };

            let (weight_recent, weight_hist) = if shift_percentage > shock_threshold {
                warn!("🚨 [SHOCK DETECTED] {} shifted {:.2}%. Engaging Reactive Weighting.", sku, shift_percentage * 100.0);
                (0.90, 0.10)
            } else {
                (0.30, 0.70)
            };

            let mut final_buy = (b_24h * weight_recent) + (b_7d * weight_hist);
            let mut final_sell = (s_24h * weight_recent) + (s_7d * weight_hist);

            // Safety Net: Prevent crossed spreads (buying higher than selling)
            if final_buy >= final_sell {
                let mid = (final_buy + final_sell) / 2.0;
                final_buy = mid * 0.98; // Force a 2% spread minimum
                final_sell = mid * 1.02;
            }

            return Some(MarketSpread {
                buy_metal: final_buy as f32,
                sell_metal: final_sell as f32,
            });
        }
    }
    None 
}

pub async fn get_24h_key_median(pool: &PgPool) -> Option<MarketSpread> {
    get_adaptive_median("5021;6", 7, 50, pool).await
}