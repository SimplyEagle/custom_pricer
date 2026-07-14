use sqlx::{PgPool, Row};

/// Fetches the rolling median price for a specific SKU over a given time window.
/// Stitches together 30 days of raw listings and long-term daily rollups.
pub async fn get_adaptive_median(
    sku: &str,
    lookback_days: i32,
    min_volume: i64,
    pool: &PgPool,
) -> Option<f32> {
    
    // The "Stitching" Query
    let query = r#"
        SELECT 
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) as median_price,
            SUM(volume) as volume
        FROM (
            -- 1. Get recent raw listings (up to 30 days, bounded by lookback_days)
            SELECT price_total_metal as price, 1 as volume 
            FROM historical_listings 
            WHERE sku = $1 
            AND created_at >= NOW() - ($2::int * INTERVAL '1 day')
            
            UNION ALL
            
            -- 2. Get historical rollups (data older than 30 days, bounded by lookback_days)
            -- We expand the daily volume to represent the historical data points
            SELECT median_price as price, volume 
            FROM historical_rollups 
            WHERE sku = $1 
            AND record_date >= NOW() - ($2::int * INTERVAL '1 day')
        ) combined_data;
    "#;

    // Execute the query securely binding both parameters
    if let Ok(row) = sqlx::query(query)
        .bind(sku)
        .bind(lookback_days)
        .fetch_one(pool)
        .await 
    {
        let volume: i64 = row.try_get("volume").unwrap_or(0);
        
        if volume >= min_volume {
            let median: f64 = row.try_get("median_price").unwrap_or(0.0);
            return Some(median as f32);
        } else {
            tracing::debug!("⚠️ [DB] Insufficient volume for {}. Found: {}/{}", sku, volume, min_volume);
        }
    }

    None 
}

pub async fn get_24h_key_median(pool: &PgPool) -> Option<f32> {
    get_adaptive_median("5021;6", 1, 50, pool).await
}