use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::Utc;
use serde::Deserialize;
use crate::state::AppState;
use crate::currency::{Currency, to_tf2_currency};
use crate::engine::calculate_market_price;

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

/// Represents the top-level websocket message from backpack.tf
#[derive(Deserialize, Debug)]
pub struct WsMessage {
    pub event: String,
    pub payload: Option<serde_json::Value>,
}

/// The inner payload detailing the specific item and price
#[derive(Deserialize, Debug)]
pub struct ListingPayload {
    pub item: Item,
    pub intent: String, // "buy" or "sell"
    pub currencies: Currencies,
}

#[derive(Deserialize, Debug)]
pub struct Item {
    pub sku: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Currencies {
    pub keys: Option<i32>,
    pub metal: Option<f32>,
}

pub async fn get_price(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PricerRequest>,
) -> Json<PricerResponse> {
    
    // Check if system is in lockdown
    let is_lockdown = { *state.is_lockdown.read().unwrap() };
    let current_key_value = { *state.live_key_price_metal.read().unwrap() };
    
    if is_lockdown {
        tracing::warn!("🚨 Rejecting price calculation for {} due to lockdown.", payload.sku);
        // Returning 0 keys/metal will cause tf2autobot to ignore the item
        return Json(build_response(&payload.sku, 0.0, 0.0, 60.0, "lockdown"));
    }

    let pool = &state.db_pool;

    // Ask the Engine to calculate the true market spread (no more hardcoded multipliers)
    let (buy_metal, sell_metal, source_marker) = match calculate_market_price(&payload.sku, pool, current_key_value).await {
        Some(market_data) => (market_data.buy_metal, market_data.sell_metal, market_data.source),
        None => (0.0, 0.0, "Failed to Price".to_string()),
    };

    Json(build_response(
        &payload.sku,
        buy_metal,
        sell_metal,
        current_key_value,
        &source_marker
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
