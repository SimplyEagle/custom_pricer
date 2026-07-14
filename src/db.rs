use sqlx::{PgPool, Row};

/// Fetches the rolling median price, blending recent and historical data,
/// and applies Shock Detection to react instantly to market crashes/spikes.
pub async fn get_adaptive_median(
    sku: &str,
    lookback_days: i32,
    min_volume: i64,
    pool: &PgPool,
) -> Option<f32> {
    
    // The "Stitching" Query with Timeframe Filters
    let query = r#"
        SELECT 
            -- 1. The Immediate Trend (Last 24h)
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= 1) as median_24h,
            -- 2. The Standard Baseline (Last 7d)
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= 7) as median_7d,
            -- 3. The Long-Term Context (Requested Lookback)
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price) FILTER (WHERE age_days <= $2::int) as median_long,
            
            SUM(volume) as total_volume
        FROM (
            SELECT price_total_metal as price, 1 as volume, EXTRACT(EPOCH FROM (NOW() - created_at))/86400 as age_days 
            FROM historical_listings 
            WHERE sku = $1 AND created_at >= NOW() - ($2::int * INTERVAL '1 day')
            
            UNION ALL
            
            SELECT median_price as price, volume, EXTRACT(EPOCH FROM (NOW() - record_date))/86400 as age_days 
            FROM historical_rollups 
            WHERE sku = $1 AND record_date >= NOW() - ($2::int * INTERVAL '1 day')
        ) combined_data;
    "#;

    if let Ok(row) = sqlx::query(query)
        .bind(sku)
        .bind(lookback_days)
        .fetch_one(pool)
        .await 
    {
        let volume: i64 = row.try_get("total_volume").unwrap_or(0);
        
        if volume >= min_volume {
            // Extract the medians. If recent data is missing (e.g., rare unusuals), it gracefully inherits older baselines.
            let m_long: f64 = row.try_get("median_long").unwrap_or(0.0);
            let m_7d: f64 = row.try_get("median_7d").unwrap_or(m_long);
            let m_24h: f64 = row.try_get("median_24h").unwrap_or(m_7d);

            if m_long == 0.0 { return None; } // Complete lack of data failsafe

            // --- SHOCK DETECTION MATH ENGINE ---
            let shock_threshold = if sku == "5021;6" {
                0.04 // 4% threshold strictly for Keys
            } else {
                0.15 // 15% threshold for standard weapons and cosmetics
            };

            // Calculate the actual market shift between recent (24h) and baseline (7d)
            let shift_percentage = if m_7d > 0.0 {
                (m_24h - m_7d).abs() / m_7d
            } else {
                0.0
            };

            let final_price = if shift_percentage > shock_threshold {
                tracing::warn!("🚨 [SHOCK DETECTED] {} moved by {:.2}%. Switching to Reactive Weighting.", sku, shift_percentage * 100.0);
                // Reactive: 90% Recent Data, 10% Historical Data
                (m_24h * 0.90) + (m_7d * 0.10)
            } else {
                // Stable: 30% Recent Data, 70% Historical Data
                (m_24h * 0.30) + (m_7d * 0.70)
            };

            return Some(final_price as f32);
        } else {
            tracing::debug!("⚠️ [DB] Insufficient volume for {}. Found: {}/{}", sku, volume, min_volume);
        }
    }

    None 
}

/// Helper function to quickly pull the key baseline
pub async fn get_24h_key_median(pool: &PgPool) -> Option<f32> {
    get_adaptive_median("5021;6", 7, 50, pool).await
}