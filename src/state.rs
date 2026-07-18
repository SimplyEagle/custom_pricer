use sqlx::postgres::PgPool;
use std::sync::RwLock;
use crate::schema::SchemaMap;

pub struct AppState {
    pub db_pool: PgPool,
    pub live_key_price_metal: RwLock<f32>,
    pub is_lockdown: RwLock<bool>,
    pub schema: SchemaMap,
}

impl AppState {
    pub fn new(db_pool: PgPool, initial_key_price: f32, schema: SchemaMap) -> Self {
        Self {
            db_pool,
            live_key_price_metal: RwLock::new(initial_key_price),
            is_lockdown: RwLock::new(false),
            schema,
        }
    }
}