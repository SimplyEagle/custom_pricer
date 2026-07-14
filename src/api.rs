use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::Utc;

use crate::state::AppState;
use crate::currency::{Currency, to_tf2_currency};
use crate::engine::calculate_item_value;

#[derive(Deserialize)]
pub struct PricerRequest {
    pub sku: String,
}

/// Formatted perfectly for TF2Autobot's custom pricer consumption
#[derive(Serialize)]
pub struct PricerResponse {
    pub sku: String,
    pub name: Option<String>,
    pub buy: Currency,
    pub sell: Currency,
    pub source: String,
    pub time: i64,
}

pub async fn get_price(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PricerRequest>,
) -> Json<PricerResponse> {
    
    // Check if system is in lockdown
    let is_lockdown = { *state.is_lockdown.read().unwrap() };
    if is_lockdown {
        println!("🚨 Rejecting price calculation for {} due to lockdown.", payload.sku);
        // Returning 0 keys/metal will cause tf2autobot to ignore the item
        return Json(build_response(&payload.sku, 0.0, 0.0, 60.0, "lockdown"));
    }

    // 1. Get the current key value from heap
    let current_key_value = { *state.live_key_price_metal.read().unwrap() };

    // 2. Ask the Engine to calculate the raw metal value
    let base_value_metal = match calculate_item_value(&payload.sku, Arc::clone(&state)).await {
        Ok(val) => val,
        Err(_) => 0.0, // Error handling
    };

    // 3. Apply Profit Margins (e.g., Buy at 95%, Sell at 105%)
    let buy_value_metal = base_value_metal * 0.95;
    let sell_value_metal = base_value_metal * 1.05;

    Json(build_response(
        &payload.sku, 
        buy_value_metal, 
        sell_value_metal, 
        current_key_value, 
        "rust_custom_pricer"
    ))
}

fn build_response(sku: &str, buy_metal: f32, sell_metal: f32, key_val: f32, source: &str) -> PricerResponse {
    PricerResponse {
        sku: sku.to_string(),
        name: None, // Optional: You can resolve SKU to string names here
        buy: to_tf2_currency(buy_metal, key_val),
        sell: to_tf2_currency(sell_metal, key_val),
        source: source.to_string(),
        time: Utc::now().timestamp(),
    }
}