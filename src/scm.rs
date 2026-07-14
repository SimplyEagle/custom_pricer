use reqwest::Client;
use serde_json::Value;
use tracing::{info, warn};

/// Fetches the fiat price of an item from the SCM and converts it to Refined Metal
pub async fn fetch_fallback_price(sku: &str, live_key_metal_price: f32) -> Option<f32> {
    
    // 1. Translate SKU to Market Hash Name
    let market_hash_name = match sku {
        "5021;6" => "Mann Co. Supply Crate Key",
        "725;6" => "Tour of Duty Ticket",
        "728;6" => "Squad Surplus Voucher",
        _ => {
            warn!("⚠️ [SCM] Name resolution for SKU {} not implemented yet.", sku);
            return None; 
        }
    };

    let client = Client::new();
    
    // 2. Fetch the baseline Key Fiat Price ($)
    let key_fiat = fetch_fiat_price("Mann Co. Supply Crate Key", &client).await?;
    
    // 3. Fetch the target Item Fiat Price ($)
    let item_fiat = fetch_fiat_price(market_hash_name, &client).await?;

    // 4. Execute Fiat-to-Metal Conversion Math
    let value_in_keys = item_fiat / key_fiat;
    let final_metal_value = value_in_keys * live_key_metal_price;

    info!("🚂 [SCM] Converted ${:.2} fiat to {:.2} ref for {}", item_fiat, final_metal_value, sku);
    
    Some(final_metal_value)
}

/// Helper function to hit the official Steam API and parse the lowest listing
async fn fetch_fiat_price(market_hash_name: &str, client: &Client) -> Option<f32> {
    let url = format!(
        "https://steamcommunity.com/market/priceoverview/?appid=440&currency=1&market_hash_name={}",
        urlencoding::encode(market_hash_name)
    );

    let res = client.get(&url).send().await.ok()?;
    let text = res.text().await.ok()?;
    let json: Value = serde_json::from_str(&text).ok()?;

    if json["success"].as_bool() == Some(true) {
        if let Some(lowest_price) = json["lowest_price"].as_str() {
            // SCM returns strings like "$2.35" or "1,200.50". We must strip the symbols to parse the float.
            let clean_price = lowest_price.replace('$', "").replace(',', "");
            return clean_price.parse::<f32>().ok();
        }
    }
    
    warn!("⚠️ [SCM] Failed to find market data for {}", market_hash_name);
    None
}