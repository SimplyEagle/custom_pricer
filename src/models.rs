use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents the top-level websocket message from backpack.tf
#[derive(Deserialize, Debug)]
pub struct WsMessage {
    pub event: String,
    pub payload: Option<ListingPayload>,
}

/// The inner payload detailing the specific item and price.
/// We use `serde_json::Value` for the item to safely parse root-level booleans 
/// (like `australium` and `craftable`) without crashing on payload shape variations.
#[derive(Deserialize, Debug)]
pub struct ListingPayload {
    pub item: Value,
    pub intent: String, // "buy" or "sell"
    pub currencies: Currencies,
}

#[derive(Deserialize, Debug, Default)]
pub struct Currencies {
    pub keys: Option<i32>,
    pub metal: Option<f32>,
}

/// The exact nested currency structure required by tf2autobot's intake.
#[derive(Serialize, Clone, Debug)]
pub struct Currency {
    pub keys: i32,
    pub metal: f32,
}

/// Represents the final unified buy/sell spread returned by the database.
#[derive(Debug, Clone)]
pub struct MarketSpread {
    pub buy_metal: f32,
    pub sell_metal: f32,
    pub volume: i64,
}