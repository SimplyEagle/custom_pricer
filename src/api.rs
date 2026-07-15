use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::Utc;

use crate::state::AppState;
use crate::currency::{Currency, to_tf2_currency};
use crate::engine;

#[derive(Deserialize)]
pub struct PricerRequest {
    pub sku: String,
}

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
    let is_lockdown = { *state.is_lockdown.read().unwrap() };
    if is_lockdown {
        tracing::warn!("🚨 Rejecting price calculation for {} due to lockdown.", payload.sku);
        return Json(build_response(&payload.sku, 0.0, 0.0, 60.0, "Lockdown Mode"));
    }

    let current_key_value = { *state.live_key_price_metal.read().unwrap() };
    let pool = &state.db_pool;

    // Use the new calculate_market_price engine that returns a MarketData struct
    if let Some(market_data) = engine::calculate_market_price(&payload.sku, pool, current_key_value).await {
        return Json(build_response(
            &payload.sku, 
            market_data.buy_metal, 
            market_data.sell_metal, 
            current_key_value, 
            &market_data.source
        ));
    }

    tracing::warn!("⚠️ [API] Could not price {}. Returning zeroed response.", payload.sku);
    Json(build_response(&payload.sku, 0.0, 0.0, current_key_value, "Error: Unpriced"))
}

fn build_response(sku: &str, buy_metal: f32, sell_metal: f32, key_val: f32, source: &str) -> PricerResponse {
    PricerResponse {
        sku: sku.to_string(),
        name: None, 
        buy: to_tf2_currency(buy_metal, key_val),
        sell: to_tf2_currency(sell_metal, key_val),
        source: source.to_string(),
        time: Utc::now().timestamp(),
    }
}