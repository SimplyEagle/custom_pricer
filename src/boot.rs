use sqlx::PgPool;
use crate::db;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct PriceDbItem {
    sell: Option<PriceDbCurrency>,
}

#[derive(Deserialize, Debug)]
struct PriceDbCurrency {
    metal: f32,
}

/// Executes the 4-Tier cascade to ensure we have a safe key price on boot
pub async fn initialize_key_price(db_pool: &PgPool) -> f32 {
    println!("🔄 Initiating 4-Tier Boot Sequence...");

    // Tier 1: Local Rolling Median
    println!("📊 Tier 1: Calculating rolling median from local database...");
    
    // Use the updated get_adaptive_median function for Keys (5021;6)
    if let Some(spread) = db::get_adaptive_median("5021;6", 7, 50, db_pool).await {
        // Calculate the exact midpoint of the market spread to act as the global Key Value
        let key_midpoint = (spread.buy_metal + spread.sell_metal) / 2.0; 
        
        println!("✅ Tier 1 Success: Loaded local median key price: {:.2} ref", key_midpoint);
        return key_midpoint;
    }
    println!("⚠️ Tier 1 Failed: Insufficient local data for a reliable median.");

    // Tier 2: pricedb.io API
    println!("🌐 Tier 2: Fetching primary fallback from pricedb.io...");
    match fetch_pricedb_key_price().await {
        Ok(api_price) => {
            println!("✅ Tier 2 Success: Loaded pricedb.io key price: {} ref", api_price);
            return api_price;
        },
        Err(_) => println!("❌ Tier 2 Failed: pricedb.io is unreachable."),
    }

    // Tier 3: Steam Community Market (SCM)
    println!("🚂 Tier 3: Fetching ultimate fallback from Steam Community Market...");
    match fetch_scm_key_to_metal_ratio().await {
        Ok(scm_price) => {
            println!("✅ Tier 3 Success: Calculated SCM key price: {} ref", scm_price);
            return scm_price;
        },
        Err(_) => println!("❌ Tier 3 Failed: SCM is unreachable."),
    }

    // Tier 4: Lockdown Mode
    println!("🚨 CRITICAL: All pricing tiers failed. System will boot in lockdown mode.");
    60.00 // Returns a baseline, but the AppState `is_lockdown` flag should be flipped true in main
}

// --- The Real Tier 2 API Call ---
async fn fetch_pricedb_key_price() -> Result<f32, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let response = client.get("https://api.pricedb.io/v1/items/5021;6")
        .send()
        .await?;

    if response.status().is_success() {
        // Read as plain text first to bypass the reqwest json feature requirement
        let text = response.text().await?;
        
        // Parse the text into JSON using serde_json
        let item_data: PriceDbItem = serde_json::from_str(&text)?;
        
        if let Some(sell_data) = item_data.sell {
            return Ok(sell_data.metal);
        }
    }
    
    Err("Failed to parse valid key price from pricedb.io".into())
}

// --- Leave Tier 3 Mocked for now until we build the Steam integration ---
async fn fetch_scm_key_to_metal_ratio() -> Result<f32, Box<dyn std::error::Error>> {
    Ok(73.55) // Simulated SCM calculation success
}