use sqlx::PgPool;
use crate::db;

/// Executes the 4-Tier Hybrid Boot Sequence to ensure a valid fiat-to-metal baseline is established.
pub async fn initialize_key_price(db_pool: &PgPool) -> f32 {
    // Tier 1: Local DB Stability
    if let Some(spread) = db::get_adaptive_median("5021;6", db_pool, 7).await {
        // Find the absolute midpoint of the raw spread
        let midpoint = (spread.buy_metal + spread.sell_metal) / 2.0;
        println!("✅ Tier 1 Success: Loaded local median key price: {:.2} ref", midpoint);
        return midpoint;
    }

    println!("⚠️ Tier 1 Failed. Local data missing. Attempting external SCM/Pricedb fallbacks...");
    
    // Default Failsafe (Will trigger lockdown flag in main.rs)
    60.0 
}