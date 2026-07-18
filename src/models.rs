use serde::{Deserialize, Serialize};

// --------------------------------------------------------
// INCOMING: Backpack.tf Websocket Payload
// --------------------------------------------------------
#[derive(Deserialize, Debug)]
pub struct WsMessage {
    pub id: Option<String>,
    pub event: Option<String>,
    pub payload: serde_json::Value, // Uses serde_json::Value to prevent array/struct crashes on error messages
}

// --------------------------------------------------------
// INCOMING: API Route Request from tf2autobot
// --------------------------------------------------------
#[derive(Deserialize, Debug)]
pub struct PricerRequest {
    pub sku: String,
}

// --------------------------------------------------------
// OUTGOING: API Route Response to tf2autobot
// --------------------------------------------------------
#[derive(Serialize, Debug)]
pub struct PricerResponse {
    pub sku: String,
    pub name: Option<String>,
    pub buy: Currency,
    pub sell: Currency,
    pub source: String,
    pub time: i64,
}

#[derive(Serialize, Debug)]
pub struct Currency {
    pub keys: i32,
    pub metal: f32,
}