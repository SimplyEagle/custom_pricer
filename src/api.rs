use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::models::Currency;
use crate::engine::calculate_market_price;
use crate::state::AppState;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct PricerRequest {
    pub sku: String,
}

/// The exact JSON contract `tf2autobot-pricedb` expects.
/// Replaces the deprecated flat `buy_half_scrap` integers with proper Currency objects.
#[derive(Serialize)]
pub struct PricerResponse {
    pub sku: String,
    pub name: String,
    pub buy: Currency,
    pub sell: Currency,
    pub source: String, // Tracks if this was a DB Match, Trait Deconstruction, or API Fallback
    pub time: i64,      // Unix timestamp for price age tracking
}

pub async fn get_price(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PricerRequest>,
) -> Json<Option<PricerResponse>> {
    let sku = payload.sku;
    
    // 🚨 FIX: MarketData is a struct, not a tuple. Unpack it correctly!
    if let Some(market_data) = calculate_market_price(&sku, &state.db_pool, state.live_key_price_metal).await {
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Ensure metal is snapped to standard TF2 increments (0.11, 0.22, etc.)
        let response = PricerResponse {
            sku: sku.clone(),
            name: format!("Calculated {}", sku), // Optional: translate via schema in production
            buy: Currency {
                keys: (market_data.buy_metal / state.live_key_price_metal).floor() as i32,
                metal: snap_to_scrap(market_data.buy_metal % state.live_key_price_metal),
            },
            sell: Currency {
                keys: (market_data.sell_metal / state.live_key_price_metal).floor() as i32,
                metal: snap_to_scrap(market_data.sell_metal % state.live_key_price_metal),
            },
            source: market_data.source,
            time: current_time,
        };

        return Json(Some(response));
    }

    Json(None)
}

/// Helper function to mathematically snap floating point metal to standard TF2 ref values.
fn snap_to_scrap(metal: f32) -> f32 {
    (metal * 9.0).round() / 9.0
}