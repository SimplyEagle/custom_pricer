use serde::Deserialize;
use tracing::{debug, error, info};
use crate::schema::SchemaMap;

#[derive(Deserialize, Debug)]
struct ScmResponse {
    success: bool,
    lowest_price: Option<String>,
}

/// Fetches the fiat value (in USD) of an item from the Steam Community Market
async fn fetch_scm_item_fiat_value(market_hash_name: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let url = format!(
        "https://steamcommunity.com/market/priceoverview/?appid=440&currency=1&market_hash_name={}",
        urlencoding::encode(market_hash_name)
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Err(format!("SCM API returned HTTP status {}", response.status()).into());
    }

    let text = response.text().await?;
    let data: ScmResponse = serde_json::from_str(&text)?;

    if !data.success {
        return Err("SCM returned success: false".into());
    }

    if let Some(price_str) = data.lowest_price {
        // Price comes in as a string like "$2.50" or "2,50€". 
        // We strip symbols and commas to isolate the float value.
        let clean_price: String = price_str.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
        if let Ok(val) = clean_price.parse::<f32>() {
            return Ok(val);
        }
    }

    Err("No valid lowest_price found in SCM response".into())
}

/// The Ultimate Safety Net: Translate SKU, fetch SCM fiat, convert to metal spread
pub async fn fetch_fallback_price(sku: &str, live_key_price_metal: f32, schema: &SchemaMap) -> Option<f32> {
    // 1. Hardcoded bypass for Keys to prevent unnecessary SCM spam loops
    if sku == "5021;6" {
        debug!("🔑 [SCM] SKU is Key, bypassing SCM Fallback.");
        return None;
    }

    // 2. Translate numerical SKU into English SCM name
    let market_hash_name = match schema.sku_to_scm_name(sku) {
        Some(name) => name,
        None => {
            error!("❌ [SCM] Failed to translate SKU {} to English. Missing from schema?", sku);
            return None;
        }
    };

    info!("🌐 [SCM] Requesting Steam Market price for: {}", market_hash_name);

    // 3. Fetch Fiat Value for the target item
    let item_fiat_value = match fetch_scm_item_fiat_value(&market_hash_name).await {
        Ok(val) => val,
        Err(e) => {
            error!("❌ [SCM] Failed to fetch item price from Steam: {}", e);
            return None;
        }
    };

    // 4. Fetch Fiat Value for a Key to establish the conversion ratio
    let key_fiat_value = match fetch_scm_item_fiat_value("Mann Co. Supply Crate Key").await {
        Ok(val) => val,
        Err(e) => {
            error!("❌ [SCM] Failed to fetch Key price for fiat conversion: {}", e);
            return None;
        }
    };

    if key_fiat_value <= 0.0 {
        return None;
    }

    // 5. Fiat-to-Metal Conversion Math
    let item_value_in_keys = item_fiat_value / key_fiat_value;
    let item_value_in_metal = item_value_in_keys * live_key_price_metal;

    info!("✅ [SCM] Fallback Success: {} is worth ${:.2} (Key is ${:.2}). Calculated Base Metal: {:.2} ref",
        market_hash_name, item_fiat_value, key_fiat_value, item_value_in_metal);

    Some(item_value_in_metal)
}