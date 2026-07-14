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
pub fn to_tf2_currency(total_metal: f32, current_key_price: f32) -> Currency {
    // Determine how many full keys fit into the total metal
    let keys = (total_metal / current_key_price).floor() as i32;
    
    // The remainder is the metal amount
    let remainder_metal = total_metal % current_key_price;
    
    Currency {
        keys,
        metal: format_metal(remainder_metal),
    }
}