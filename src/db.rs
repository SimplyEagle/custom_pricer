use sqlx::PgPool;

pub struct MarketSpread {
    pub buy_metal: f32,
    pub sell_metal: f32,
}

/// Queries PostgreSQL using Time-Series Downsampling and CROSS JOIN generation to perfectly weight median calculations
pub async fn get_adaptive_median(sku: &str, pool: &PgPool, lookback_days: i32) -> Option<MarketSpread> {
    let query = r#"
    WITH stitched_data AS (
        -- 1. Raw recent transactions
        SELECT intent, price_total_metal
        FROM historical_listings
        WHERE sku = $1 AND created_at >= NOW() - ($2 * INTERVAL '1 day')

        UNION ALL

        -- 2. Dynamically unpack compressed rollups using CROSS JOIN generate_series
        SELECT intent, median_price as price_total_metal
        FROM historical_rollups
        CROSS JOIN generate_series(1, volume)
        WHERE sku = $1 AND record_date >= NOW() - ($2 * INTERVAL '1 day')
    )
    SELECT
        COALESCE((SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) FROM stitched_data WHERE intent = 'buy'), 0.0) as buy_median,
        COALESCE((SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) FROM stitched_data WHERE intent = 'sell'), 0.0) as sell_median
    "#;

    if let Ok(row) = sqlx::query(query)
        .bind(sku)
        .bind(lookback_days)
        .fetch_one(pool)
        .await
    {
        let buy_metal: f64 = sqlx::Row::try_get(&row, "buy_median").unwrap_or(0.0);
        let sell_metal: f64 = sqlx::Row::try_get(&row, "sell_median").unwrap_or(0.0);

        if buy_metal > 0.0 && sell_metal > 0.0 {
            return Some(MarketSpread {
                buy_metal: buy_metal as f32,
                sell_metal: sell_metal as f32,
            });
        }
    }
    None
}