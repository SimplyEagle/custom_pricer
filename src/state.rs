use sqlx::PgPool;
use std::sync::RwLock;

/// Shared application state accessible by all Axum routes
pub struct AppState {
    /// Database connection pool
    pub db_pool: PgPool,
    
    /// Thread-safe live key price in Refined metal
    pub live_key_price_metal: RwLock<f32>,
    
    /// If true, the bot will return 0 confidence to prevent trading
    pub is_lockdown: RwLock<bool>,
}

impl AppState {
    pub fn new(db_pool: PgPool, initial_key_price: f32) -> Self {
        Self {
            db_pool,
            live_key_price_metal: RwLock::new(initial_key_price),
            is_lockdown: RwLock::new(false),
        }
    }
}