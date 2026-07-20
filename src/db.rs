use sqlx::PgPool;
use tracing::error;
use crate::models::MarketSpread;

/// Executes a mathematically correct weighted-stitching query to pull historical spreads.
pub async fn get_adaptive_median(
    pool: &PgPool, 
    sku: &str, 
    lookback_days: i32
) -> Option<MarketSpread> {
    
    // The `generate_series` CROSS JOIN physically duplicates the rolled-up row in Postgres memory 
    // based on its volume. This guarantees a compressed row representing 5,000 trades 
    // exerts 5,000x more mathematical pull on the `percentile_cont` median than a single raw outlier.
    let query = r#"
        WITH combined_data AS (
            SELECT intent, price_total_metal
            FROM historical_listings
            WHERE sku = $1 AND created_at >= NOW() - ($2 || ' days')::INTERVAL

            UNION ALL

            SELECT r.intent, r.median_price as price_total_metal
            FROM historical_rollups r
            CROSS JOIN generate_series(1, r.volume::integer)
            WHERE r.sku = $1 AND r.record_date >= NOW() - ($2 || ' days')::INTERVAL
        )
        SELECT 
            COALESCE((SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) FROM combined_data WHERE intent = 'buy'), 0.0) as buy_metal,
            COALESCE((SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) FROM combined_data WHERE intent = 'sell'), 0.0) as sell_metal,
            COUNT(*) as volume
        FROM combined_data;
    "#;

    let result = sqlx::query_as::<_, (f64, f64, i64)>(query)
        .bind(sku)
        .bind(lookback_days)
        .fetch_one(pool)
        .await;

    match result {
        Ok((buy, sell, vol)) if vol > 0 => {
            Some(MarketSpread {
                buy_metal: buy as f32,
                sell_metal: sell as f32,
                volume: vol,
            })
        },
        Ok(_) => None, // Returns none if no rows existed, triggering your fallbacks
        Err(e) => {
            error!("Database query failed for SKU {}: {:?}", sku, e);
            None
        }
    }
}

/// Boot handler used specifically to fetch raw key statistics on startup
pub async fn get_24h_key_median(pool: &PgPool) -> Option<MarketSpread> {
    get_adaptive_median(pool, "5021;6", 1).await
}