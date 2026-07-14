//! Handles component-based pricing for low-liquidity items (Unusuals & Pro KS)

/// TF2 Sheen IDs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sheen {
    TeamShine = 1,
    DeadlyDaffodil = 2,
    Manndarin = 3,
    AgonizingEmerald = 4,
    VillainousViolet = 5,
    HotRod = 6,
    MeanGreen = 7,
    Unknown,
}

impl Sheen {
    pub fn from_id(id: i32) -> Self {
        match id {
            1 => Sheen::TeamShine,
            2 => Sheen::DeadlyDaffodil,
            3 => Sheen::Manndarin,
            4 => Sheen::AgonizingEmerald,
            5 => Sheen::VillainousViolet,
            6 => Sheen::HotRod,
            7 => Sheen::MeanGreen,
            _ => Sheen::Unknown,
        }
    }

    /// Returns the market multiplier for this specific sheen
    pub fn market_multiplier(&self) -> f32 {
        match self {
            Sheen::TeamShine => 1.35,
            Sheen::VillainousViolet | Sheen::DeadlyDaffodil => 1.15,
            Sheen::Manndarin | Sheen::AgonizingEmerald | Sheen::HotRod => 1.0,
            Sheen::MeanGreen | Sheen::Unknown => 0.85, // Heavily penalized
        }
    }
}

/// TF2 Killstreaker IDs
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Killstreaker {
    FireHorns = 2002,
    CerebralDischarge = 2003,
    Tornado = 2004,
    Flames = 2005,
    Singularity = 2006,
    Incinerator = 2007,
    HypnoBeam = 2008,
    Unknown,
}

impl Killstreaker {
    pub fn from_id(id: i32) -> Self {
        match id {
            2002 => Killstreaker::FireHorns,
            2003 => Killstreaker::CerebralDischarge,
            2004 => Killstreaker::Tornado,
            2005 => Killstreaker::Flames,
            2006 => Killstreaker::Singularity,
            2007 => Killstreaker::Incinerator,
            2008 => Killstreaker::HypnoBeam,
            _ => Killstreaker::Unknown,
        }
    }

    /// Returns the market multiplier for this specific killstreaker
    pub fn market_multiplier(&self) -> f32 {
        match self {
            Killstreaker::FireHorns | Killstreaker::Flames => 1.40,
            Killstreaker::Tornado => 1.20,
            Killstreaker::Singularity | Killstreaker::CerebralDischarge => 1.0,
            Killstreaker::Incinerator | Killstreaker::Unknown => 0.90,
            Killstreaker::HypnoBeam => 0.75, // The market hates Hypno-Beam
        }
    }
}

/// Parses a SKU and calculates the Professional Killstreak premium.
/// Example SKU: "200;11;kt-2002;ks-1" (Pro KS Rocket Launcher, Fire Horns, Team Shine)
pub fn calculate_pro_ks_premium(sku: &str, generic_kit_metal_value: f32) -> f32 {
    let mut sheen = Sheen::Unknown;
    let mut streaker = Killstreaker::Unknown;

    // Parse the SKU string for the traits
    for part in sku.split(';') {
        if let Some(stripped) = part.strip_prefix("ks-") {
            if let Ok(id) = stripped.parse::<i32>() {
                sheen = Sheen::from_id(id);
            }
        }
        if let Some(stripped) = part.strip_prefix("kt-") {
            if let Ok(id) = stripped.parse::<i32>() {
                streaker = Killstreaker::from_id(id);
            }
        }
    }

    // Apply the matrix calculation
    let sheen_mult = sheen.market_multiplier();
    let streaker_mult = streaker.market_multiplier();
    
    // The final premium is the baseline kit value scaled by the desirability of the combo
    generic_kit_metal_value * sheen_mult * streaker_mult
}

/// Calculates the Unusual premium based on the historical median of the Effect ID itself
/// rather than relying on exact Hat+Effect combo historical data.
pub fn calculate_unusual_premium(
    base_hat_value_metal: f32, 
    effect_median_metal_value: f32,
    is_cancer_hat: bool // You could determine this via a static list of undesirable defindexes
) -> f32 {
    if is_cancer_hat {
        // For terrible hats, the effect is the only thing of value. Additive scaling.
        base_hat_value_metal + (effect_median_metal_value * 0.9)
    } else {
        // For standard or high-tier hats, the synergy creates a multiplicative increase.
        // A 20% synergy bonus makes high-tier hats with good effects price appropriately.
        base_hat_value_metal + (effect_median_metal_value * 1.20)
    }
}

/// Translates an applied Strange Part Attribute ID into its unapplied Item Defindex
/// so the engine can look up its market value in the database.
pub fn get_strange_part_defindex(attribute_id: i64) -> Option<i64> {
    match attribute_id {
        // --- Wave 1 & 2 ---
        388 => Some(6002), // Heavies Killed
        389 => Some(6003), // Buildings Destroyed
        390 => Some(6004), // Projectiles Reflected
        391 => Some(6005), // Headshot Kills
        392 => Some(6006), // Airborne Enemies Killed
        393 => Some(6007), // Gib Kills
        394 => Some(6008), // Buildings Sapped
        395 => Some(6009), // Spies Killed
        396 => Some(6010), // Snipers Killed
        397 => Some(6011), // Demomen Killed
        398 => Some(6012), // Scouts Killed
        399 => Some(6013), // Medics Killed
        400 => Some(6014), // Engineers Killed
        401 => Some(6015), // Pyros Killed
        402 => Some(6016), // Soldiers Killed
        403 => Some(6017), // Domination Kills
        404 => Some(6018), // Revenge Kills
        405 => Some(6019), // Posthumous Kills
        406 => Some(6020), // Teammates Extinguished
        407 => Some(6021), // Critical Kills
        408 => Some(6022), // Kills While Explosive Jumping

        // --- Wave 3 & 4 ---
        435 => Some(6023), // Submerged Enemies Killed
        441 => Some(6024), // Defenders Killed
        442 => Some(6025), // Underwater Kills
        443 => Some(6026), // Kills While Invuln
        444 => Some(6027), // Kills While Cloaked
        445 => Some(6028), // Freezecam Taunt Appearances
        446 => Some(6029), // Damage Dealt
        447 => Some(6030), // Fires Survived
        448 => Some(6031), // Allied Healing Done
        449 => Some(6032), // Point Blank Kills
        450 => Some(6033), // Long Distance Kills
        451 => Some(6034), // Kills during Victory Time
        452 => Some(6035), // Robot Scouts Destroyed
        453 => Some(6036), // Taunt Kills
        454 => Some(6037), // Non-Crit Kills
        455 => Some(6038), // Player Hits
        456 => Some(6039), // Assists

        // --- Wave 5 (MvM & Halloween) ---
        465 => Some(6040), // Sappers Removed
        466 => Some(6041), // Cloaked Spies Killed
        467 => Some(6042), // Medics Killed That Have Full ÜberCharge
        468 => Some(6043), // Robots Destroyed
        469 => Some(6044), // Giant Robots Destroyed
        470 => Some(6045), // Robot Snipers Destroyed
        471 => Some(6046), // Robot Spies Destroyed
        472 => Some(6047), // Tanks Destroyed
        473 => Some(6048), // Kills While Low Health
        474 => Some(6049), // Halloween: Kills
        475 => Some(6050), // Halloween: Robots Destroyed
        477 => Some(6052), // Kills Under A Full Moon
        478 => Some(6053), // Dominations (Halloween)
        
        // --- Niche / Cosmetic ---
        483 => Some(6054), // Kills with a Taunt Attack
        488 => Some(6055), // Notorious Kills
        
        _ => None, // Failsafe for unmapped or standard attributes
    }
}

/// Calculates the final market price of a strange weapon by adding 20% 
/// of the unapplied value of each attached Strange Part.
pub fn calculate_strange_parts_premium(
    base_weapon_metal_value: f32, 
    strange_parts_metal_values: Vec<f32>
) -> f32 {
    let mut total_premium = 0.0;
    
    for part_value in strange_parts_metal_values {
        // Industry standard: Applied parts retain 20% of their base value
        total_premium += part_value * 0.20; 
    }

    base_weapon_metal_value + total_premium
}
