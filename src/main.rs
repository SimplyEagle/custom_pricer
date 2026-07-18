mod state;
mod currency;
mod db;
mod engine;
mod boot;
mod api;
mod websocket;
mod traits;
mod models;
mod scm;
mod schema;

use axum::{routing::post, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use sqlx::postgres::PgPoolOptions;
use tracing::{info, debug, warn, error};
use state::AppState;

fn build_sku_from_item(item: &serde_json::Value) -> Option<String> {
    let defindex = item["defindex"].as_i64()?;
    
    // Ignore invalid items
    if defindex <= 0 { return None; }
    
    // Backpack.tf can send quality as an int or an object depending on the event
    let quality = item["quality"].as_i64()
        .or_else(|| item["quality"]["id"].as_i64())
        .unwrap_or(6);
        
    let mut sku = format!("{};{}", defindex, quality);

    // --- FOOLPROOF TEXT FALLBACKS ---
    // Buy orders often strip out attributes and booleans, but ALWAYS include the English name.
    let name = item["name"].as_str()
        .or_else(|| item["market_name"].as_str())
        .or_else(|| item["baseName"].as_str())
        .unwrap_or("");

    // --- 1. TF2-SKU STANDARD ORDER ---
    let is_craftable = item["craftable"].as_bool()
        .or_else(|| item["craftable"].as_i64().map(|v| v == 1))
        .unwrap_or(!name.contains("Non-Craftable"));

    if !is_craftable {
        sku.push_str(";uncraftable");
    }

    // Catch Root Booleans, Integers, or English text
    let mut is_australium = item["australium"].as_bool().unwrap_or(false)
        || item["australium"].as_i64() == Some(1)
        || name.contains("Australium");

    let mut is_festivized = item["festivized"].as_bool().unwrap_or(false)
        || item["festivized"].as_i64() == Some(1)
        || name.contains("Festivized");

    let mut effect = None;
    let mut sheen = None;
    let mut killstreaker = None;
    let mut ks_tier = None;
    let mut paintkit = None;
    let mut wear = None;
    let mut strange_parts = Vec::new();
    let mut is_strange_elevated = false;

    // Parse traits from the attributes array
    if let Some(attributes) = item["attributes"].as_array() {
        for attr in attributes {
            let attr_def = attr["defindex"].as_i64().unwrap_or(0);
            let value = attr["value"].as_i64().or_else(|| attr["float_value"].as_f64().map(|v| v as i64));

            match attr_def {
                134 => effect = value,       // Unusual Effect ID
                725 => wear = value,         // Warpaint Wear ID
                834 => paintkit = value,     // Warpaint Paintkit ID
                2013 => sheen = value,       // Killstreak Sheen ID
                2014 => killstreaker = value,// Professional Killstreaker ID
                2025 => ks_tier = value,     // Killstreak Tier
                2027 => is_australium = true,// Australium Attribute
                2053 => is_festivized = true,// Festivized Attribute
                214 if value == Some(11) => is_strange_elevated = true,
                id if crate::traits::get_strange_part_defindex(id).is_some() => {
                    if let Some(part_defindex) = crate::traits::get_strange_part_defindex(id) {
                        strange_parts.push(part_defindex);
                    }
                },
                _ => {}
            }
        }
    }

    // Text fallback for Killstreak Tiers on Buy Orders
    if ks_tier.is_none() {
        if name.contains("Professional Killstreak") {
            ks_tier = Some(3);
        } else if name.contains("Specialized Killstreak") {
            ks_tier = Some(2);
        } else if name.contains("Killstreak") {
            ks_tier = Some(1);
        }
    }

    if is_australium {
        sku.push_str(";australium");
    }

    if is_festivized {
        sku.push_str(";festive");
    }

    if is_strange_elevated || (name.contains("Strange") && quality != 11) {
        sku.push_str(";strange");
    }

    if let Some(e) = effect {
        sku.push_str(&format!(";u{}", e));
    }

    if let Some(pk) = paintkit {
        sku.push_str(&format!(";pk{}", pk));
    }

    if let Some(w) = wear {
        sku.push_str(&format!(";w{}", w));
    }

    if let Some(kt) = ks_tier {
        sku.push_str(&format!(";kt-{}", kt));
    }

    // --- 2. CUSTOM PRICER TRAITS ---
    if let Some(sh) = sheen {
        sku.push_str(&format!(";sheen-{}", sh));
    }
    if let Some(st) = killstreaker {
        sku.push_str(&format!(";streaker-{}", st));
    }
    
    strange_parts.sort_unstable();
    for part_defindex in strange_parts {
        sku.push_str(&format!(";sp-{}", part_defindex));
    }

    Some(sku)
}

async fn run_cleanup(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    tracing::info!("🧹 [Garbage Collector] Running cleanup...");
        
    // Step A: Calculate daily medians SEPARATELY for buy and sell intents
    sqlx::query(
        r#"
        INSERT INTO historical_rollups (sku, intent, record_date, median_price, volume)
        SELECT 
            sku, 
            intent, 
            DATE(created_at) as record_date,
            percentile_cont(0.5) WITHIN GROUP (ORDER BY price_total_metal) as median_price,
            COUNT(*) as volume
        FROM historical_listings
        WHERE created_at < NOW() - INTERVAL '30 days'
        GROUP BY sku, intent, DATE(created_at)
        ON CONFLICT (sku, intent, record_date) DO NOTHING;
        "#
    )
    .execute(pool)
    .await?;
        
    let result = sqlx::query("DELETE FROM historical_listings WHERE created_at < NOW() - INTERVAL '30 days'")
        .execute(pool)
        .await?;
            
    tracing::info!("♻️ [Garbage Collector] Success. Cleared {} raw rows.", result.rows_affected());
    Ok(())
}

#[tokio::main]
async fn main() {
    // 0. Explicitly declare the cryptography backend
    rustls::crypto::ring::default_provider().install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize the tracing subscriber to read the RUST_LOG environment variable
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🔧 Booting Rust TF2 Pricer...");

    // 1. Initialize Database
    // Note: When running locally use localhost. When in docker, use the env var or 'db'.
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/tf2_market".to_string());
            
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(&database_url).expect("Failed to create pool");

    // Load Schema for SCM translations via environment variable or default
    let schema_path = std::env::var("SCHEMA_PATH").unwrap_or_else(|_| "schema.json".to_string());
    let schema_map = schema::SchemaMap::load(&schema_path);

    // 2. Run the 4-Tier Boot Sequence
    let initial_key_price = boot::initialize_key_price(&db_pool).await;

    // 3. Initialize Shared Application State
    let shared_state = Arc::new(AppState::new(db_pool.clone(), initial_key_price, schema_map));

    if initial_key_price == 60.00 {
        let mut lockdown = shared_state.is_lockdown.write().unwrap();
        *lockdown = true;
    }

    // 4. Setup internal channel for Websocket -> DB ingestion
    let (tx, mut rx) = mpsc::channel::<String>(10000);

    // 5. Spawn the Websocket Listener
    tokio::spawn(websocket::start_listener(tx));
        
    // 6. Spawn the DB Ingestion worker
    let worker_pool = db_pool.clone();
    let worker_state = Arc::clone(&shared_state);
        
    tokio::spawn(async move {
        info!("💾 [Worker] Database ingestion worker started.");
                
        while let Some(msg) = rx.recv().await {
            if let Ok(messages) = serde_json::from_str::<Vec<serde_json::Value>>(&msg) {
                for ws_msg in messages {
                    let event = ws_msg["event"].as_str().unwrap_or("");

                    if event == "listing-update" {
                        let payload = &ws_msg["payload"];
                        let intent = payload["intent"].as_str().unwrap_or("sell");

                        let keys = payload["currencies"]["keys"].as_f64().unwrap_or(0.0) as i32;
                        let metal = payload["currencies"]["metal"].as_f64().unwrap_or(0.0) as f32;

                        let item = &payload["item"];
                        
                        if let Some(sku) = build_sku_from_item(item) {
                            let live_key_val = { *worker_state.live_key_price_metal.read().unwrap() };
                            let total_metal = (keys as f32 * live_key_val) + metal;

                            let result = sqlx::query(
                                r#"
                                INSERT INTO historical_listings (sku, intent, keys, metal, price_total_metal)
                                VALUES ($1, $2, $3, $4, $5)
                                "#
                            )
                            .bind(&sku)
                            .bind(intent)
                            .bind(keys)
                            .bind(metal)
                            .bind(total_metal)
                            .execute(&worker_pool)
                            .await;

                            if let Err(e) = result {
                                error!("❌ [Worker] DB Insert Error for {}: {}", sku, e);
                            } else {
                                let name = item["name"].as_str().or_else(|| item["baseName"].as_str()).unwrap_or(&sku);
                                
                                // X-Ray vision strictly for Australiums!
                                if name.contains("Australium") {
                                    info!("🌟 [Australium Tracker] Captured: {} | Saved as SKU: {}", name, sku);
                                }

                                debug!("📥 [Worker] Saved {} {} ({} ref)", intent, name, total_metal);
                            }
                        }
                    } else if event == "client-limit-exceeded" {
                        warn!("🚨 [API LIMIT] Backpack.tf rejected the connection. Check for ghost connections!");
                    }
                }
            }
        }
    });

    // 7. Spawn the Data Rollup & Cleanup Worker (The Garbage Collector)
    let gc_pool = db_pool.clone();
    tokio::spawn(async move {
        info!("🧹 [Garbage Collector] Downsampling worker initialized.");
                
        // This runs immediately when the bot starts to clear anything missed while down
        let _ = run_cleanup(&gc_pool).await;

        // Loop forever
        loop {
            // Wake up every 24 hours
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)).await;
            info!("🧹 [Garbage Collector] Waking up to compress old data...");
            let _ = run_cleanup(&gc_pool).await;
        }
    });

    // 8. Start the Axum HTTP Server
    let app = Router::new()
        .route("/price", post(api::get_price))
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    info!("🚀 Rust Pricer API ready on http://{}", addr);
        
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}