use serde::Deserialize;

/// Represents the top-level websocket message from backpack.tf
#[derive(Deserialize, Debug)]
pub struct WsMessage {
    pub event: String,
    pub payload: Option<ListingPayload>,
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