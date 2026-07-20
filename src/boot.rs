use sqlx::PgPool;
use crate::db;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct AuthResponse {
    #[serde(rename = "accessToken")]
    access_token: String,
}

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
    if let Some(spread) = db::get_24h_key_median(db_pool).await {
        let midpoint = (spread.buy_metal + spread.sell_metal) / 2.0;
        println!("✅ Tier 1 Success: Loaded local median key price: {:.2} ref", midpoint);
        return midpoint;
    }
    
    println!("⚠️ Tier 1 Failed: Insufficient local data for a reliable median.");

    // Tier 2: pricedb.io API
    println!("🌐 Tier 2: Fetching primary fallback from pricedb.io...");
    match fetch_pricedb_key_price().await {
        Ok(api_price) => {
            println!("✅ Tier 2 Success: Loaded pricedb.io key price: {:.2} ref", api_price);
            return api_price;
        },
        Err(e) => println!("❌ Tier 2 Failed: {}", e),
    }

    // Tier 3: Steam Community Market (SCM)
    println!("🚂 Tier 3: Fetching ultimate fallback from Steam Community Market...");
    match crate::scm::fetch_scm_key_to_metal_ratio().await {
        Ok(scm_price) => {
            println!("✅ Tier 3 Success: Calculated SCM key price: {:.2} ref", scm_price);
            return scm_price;
        },
        Err(e) => println!("❌ Tier 3 Failed: {}", e),
    }

    // Tier 4: Lockdown Mode
    println!("🚨 CRITICAL: All pricing tiers failed. System will boot in lockdown mode.");
    60.00 // Returns a baseline, but the AppState is_lockdown flag should be flipped true in main
}

// --- The Real Tier 2 API Call (The TF2Autobot Handshake) ---
async fn fetch_pricedb_key_price() -> Result<f32, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .user_agent("tf2autobot@5.16.8") // The exact bot user agent
        .build()?;

    // STEP 1: The Anonymous Auth Handshake
    // We request a temporary access token from the server (No API key needed!)
    let auth_res = client.post("https://api.pricedb.io/v1/auth/access")
        .send()
        .await?;

    if !auth_res.status().is_success() {
        return Err(format!("Auth Handshake Failed. HTTP {}", auth_res.status()).into());
    }

    // Parse as text first to bypass the missing reqwest json feature
    let auth_text = auth_res.text().await?;
    let auth_data: AuthResponse = serde_json::from_str(&auth_text)?;

    // STEP 2: The Price Query
    // We attach the temporary Bearer token we just generated to authorize the GET request
    let response = client.get("https://api.pricedb.io/v1/items/5021;6")
        .bearer_auth(auth_data.access_token)
        .send()
        .await?;

    if response.status().is_success() {
        let text = response.text().await?;
        
        let item_data: PriceDbItem = match serde_json::from_str(&text) {
            Ok(data) => data,
            Err(e) => return Err(format!("JSON Parse Error: {}", e).into()),
        };

        if let Some(sell_data) = item_data.sell {
            return Ok(sell_data.metal);
        } else {
            return Err("Pricedb returned JSON, but 'sell' listing was missing.".into());
        }
    }
    
    Err(format!("Item Query Failed. HTTP {}", response.status()).into())
}