use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use tracing::{error, info};

#[derive(Debug, Default, Clone)]
pub struct SchemaMap {
    pub items: HashMap<i32, String>,
}

impl SchemaMap {
    /// Loads schema.json into memory to translate SKUs to English Market names
    pub fn load(path: &str) -> Self {
        let mut map = HashMap::new();
        
        let data = match fs::read_to_string(path) {
            Ok(d) => d,
            Err(e) => {
                error!("⚠️ [Schema] Failed to read {}: {}. SCM Fallback will fail for unnamed items.", path, e);
                return SchemaMap { items: map };
            }
        };

        let json: Value = match serde_json::from_str(&data) {
            Ok(j) => j,
            Err(e) => {
                error!("⚠️ [Schema] Failed to parse {}: {}", path, e);
                return SchemaMap { items: map };
            }
        };

        // Navigate through the tf2autobot schema.json structure
        let items_array = json.pointer("/raw/schema/items")
            .or_else(|| json.pointer("/schema/items"))
            .or_else(|| json.pointer("/items"))
            .and_then(|v| v.as_array());

        if let Some(items) = items_array {
            for item in items {
                if let (Some(defindex), Some(name)) = (item["defindex"].as_i64(), item["item_name"].as_str()) {
                    map.insert(defindex as i32, name.to_string());
                }
            }
            info!("📖 [Schema] Successfully loaded {} item names into memory.", map.len());
        } else {
            error!("⚠️ [Schema] Could not find items array in schema.json.");
        }

        SchemaMap { items: map }
    }

    /// Converts a numerical SKU string into an official Steam Community Market English name
    pub fn sku_to_scm_name(&self, sku: &str) -> Option<String> {
        let mut parts = sku.split(';');
        let defindex: i32 = parts.next()?.parse().ok()?;
        let quality: i32 = parts.next()?.parse().ok()?;

        let base_name = self.items.get(&defindex)?;

        let mut is_uncraftable = false;
        let mut is_australium = false;
        let mut is_festivized = false;
        let mut ks_tier = 0;

        for part in parts {
            if part == "uncraftable" { is_uncraftable = true; }
            if part == "australium" { is_australium = true; }
            if part == "festive" { is_festivized = true; }
            if let Some(kt) = part.strip_prefix("kt-") {
                ks_tier = kt.parse().unwrap_or(0);
            }
        }

        let mut final_name = String::new();

        // Steam enforces a strict prefix order for English names
        if is_uncraftable {
            final_name.push_str("Non-Craftable ");
        }

        match quality {
            1 => final_name.push_str("Genuine "),
            3 => final_name.push_str("Vintage "),
            5 => final_name.push_str("Unusual "), // Note: SCM requires exact effect names for unusuals, which is highly complex to map.
            11 => final_name.push_str("Strange "),
            14 => final_name.push_str("Collector's "),
            _ => {} // Unique (6) has no prefix
        }

        if is_festivized {
            final_name.push_str("Festivized ");
        }

        match ks_tier {
            1 => final_name.push_str("Killstreak "),
            2 => final_name.push_str("Specialized Killstreak "),
            3 => final_name.push_str("Professional Killstreak "),
            _ => {}
        }

        if is_australium {
            final_name.push_str("Australium ");
        }

        // Append the actual base weapon/item name
        final_name.push_str(base_name);

        Some(final_name)
    }
}