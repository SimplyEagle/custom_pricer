use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::Utc;
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

pub async fn get_price(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PricerRequest>,
) -> Json<PricerResponse> {
    
    // Check if system is in lockdown
    let is_lockdown = { *state.is_lockdown.read().unwrap() };
    let current_key_value = { *state.live_key_price_metal.read().unwrap() };

    if is_lockdown {
        tracing::warn!("🚨 Rejecting price calculation for {} due to lockdown.", payload.sku);
        return Json(build_response(&payload.sku, 0.0, 0.0, current_key_value, "lockdown"));
    }

    // Call the engine with the database pool, key value, and schema map
    let market_data = match calculate_market_price(&payload.sku, &state.db_pool, current_key_value, &state.schema).await {
        Some(data) => data,
        None => {
            tracing::warn!("❌ [API] All fallback tiers failed for {}.", payload.sku);
            return Json(build_response(&payload.sku, 0.0, 0.0, current_key_value, "unpriced"));
        }
    };

    Json(build_response(
        &payload.sku,
        market_data.buy_metal,
        market_data.sell_metal,
        current_key_value,
        &market_data.source
    ))
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