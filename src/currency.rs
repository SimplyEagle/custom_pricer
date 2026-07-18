use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Currency {
    pub keys: i32,
    pub metal: f32,
}

/// Snaps a raw float to the nearest valid TF2 metal increment (0.11, 0.22, etc.)
pub fn format_metal(raw_metal: f32) -> f32 {
    let scrap_count = (raw_metal * 9.0).round();
    let formatted = scrap_count / 9.0;
    (formatted * 100.0).round() / 100.0
}

/// Converts a total raw metal value into Keys and formatted TF2 Metal
pub fn to_tf2_currency(total_metal: f32, key_price_in_metal: f32) -> Currency {
    if total_metal <= 0.0 || key_price_in_metal <= 0.0 {
        return Currency { keys: 0, metal: 0.0 };
    }

    // Determine total keys
    let keys = (total_metal / key_price_in_metal).floor() as i32;
    
    // Determine remaining metal after keys are removed
    let remaining_metal = total_metal - (keys as f32 * key_price_in_metal);

    // Snap metal to TF2 standard formatting (e.g., 0.11, 0.22)
    let snapped_metal = snap_to_scrap(remaining_metal);

    Currency {
        keys,
        metal: snapped_metal,
    }
}

/// Rounds metal values to the nearest 0.11 increment
pub fn snap_to_scrap(metal: f32) -> f32 {
    (metal * 9.0).round() / 9.0
}
