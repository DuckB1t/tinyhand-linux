use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Style {
    pub id: &'static str,
    pub title: &'static str,
    pub ink: &'static str,
    pub asset: Option<&'static str>,
}

impl Style {
    pub fn is_laser(&self) -> bool {
        self.asset.is_none()
    }
}

pub const STYLES: &[Style] = &[
    Style { id: "lavender", title: "Lavender toy", ink: "#8754F2", asset: Some("hand_lavender") },
    Style { id: "glove", title: "Cursor glove", ink: "#8754F2", asset: Some("hand_glove") },
    Style { id: "coral", title: "Coral vinyl", ink: "#FF2920", asset: Some("hand_coral") },
    Style { id: "plush", title: "Plush", ink: "#2F86F0", asset: Some("hand_plush") },
    Style { id: "robot", title: "Robot", ink: "#16A87A", asset: Some("hand_robot") },
    Style { id: "pink", title: "Bubblegum", ink: "#EE4F98", asset: Some("hand_pink") },
    Style { id: "blue", title: "Sky", ink: "#2F86F0", asset: Some("hand_blue") },
    Style { id: "mint", title: "Mint", ink: "#16A87A", asset: Some("hand_mint") },
    Style { id: "lemon", title: "Lemon", ink: "#F2B705", asset: Some("hand_lemon") },
    Style { id: "black", title: "Midnight", ink: "#1D1D21", asset: Some("hand_black") },
    Style { id: "chrome", title: "Chrome", ink: "#6B7380", asset: Some("hand_chrome") },
    Style { id: "holo", title: "Holo", ink: "#8754F2", asset: Some("hand_holo") },
    Style { id: "fuzzy", title: "Fuzzy", ink: "#8754F2", asset: Some("hand_fuzzy") },
    Style { id: "wood", title: "Wood", ink: "#9A5E2C", asset: Some("hand_wood") },
    Style { id: "marble", title: "Marble", ink: "#6B7380", asset: Some("hand_marble") },
    Style { id: "terrazzo", title: "Terrazzo", ink: "#F0561D", asset: Some("hand_terrazzo") },
    Style { id: "glass", title: "Glass", ink: "#1597E5", asset: Some("hand_glass") },
    Style { id: "jelly", title: "Jelly", ink: "#A33FE6", asset: Some("hand_jelly") },
    Style { id: "pastel", title: "Pastel", ink: "#EE4F98", asset: Some("hand_pastel") },
    Style { id: "pixel", title: "Pixel", ink: "#1D1D21", asset: Some("hand_pixel") },
    Style { id: "lowpoly", title: "Low poly", ink: "#8754F2", asset: Some("hand_lowpoly") },
    Style { id: "knit", title: "Knit", ink: "#8754F2", asset: Some("hand_knit") },
    Style { id: "skeleton", title: "Skeleton", ink: "#9A5E2C", asset: Some("hand_skeleton") },
    Style { id: "astro", title: "Astronaut", ink: "#2F86F0", asset: Some("hand_astro") },
    Style { id: "space", title: "Space", ink: "#FF2920", asset: Some("hand_space") },
    Style { id: "monster", title: "Monster", ink: "#3E9E2E", asset: Some("hand_monster") },
    Style { id: "water", title: "Water", ink: "#1597E5", asset: Some("hand_water") },
    Style { id: "lava", title: "Lava", ink: "#F0561D", asset: Some("hand_lava") },
    Style { id: "grass", title: "Grass", ink: "#3E9E2E", asset: Some("hand_grass") },
    Style { id: "cactus", title: "Cactus", ink: "#3E9E2E", asset: Some("hand_cactus") },
    Style { id: "choco", title: "Chocolate", ink: "#9A5E2C", asset: Some("hand_choco") },
    Style { id: "cloud", title: "Cloud", ink: "#2F86F0", asset: Some("hand_cloud") },
    Style { id: "mech", title: "Mech", ink: "#F0561D", asset: Some("hand_mech") },
    Style { id: "neon", title: "Neon", ink: "#E0309A", asset: Some("hand_neon") },
    Style { id: "leaf", title: "Leaf", ink: "#3E9E2E", asset: Some("hand_leaf") },
    Style { id: "cookie", title: "Gingerbread", ink: "#9A5E2C", asset: Some("hand_cookie") },
    Style { id: "laser", title: "Laser dot", ink: "#FF2920", asset: None },
];

pub const DEFAULT_PINNED: &[&str] = &["lavender", "glove", "coral", "plush", "robot", "laser"];

pub const INK_CHOICES: &[(&str, &str, Option<&str>)] = &[
    ("match", "Match pointer", None),
    ("red", "Red", Some("#FF2920")),
    ("orange", "Orange", Some("#FF8A00")),
    ("yellow", "Yellow", Some("#FFCC00")),
    ("green", "Green", Some("#30C25A")),
    ("blue", "Blue", Some("#0A84FF")),
    ("purple", "Purple", Some("#8754F2")),
    ("white", "White", Some("#FFFFFF")),
    ("black", "Black", Some("#1D1D21")),
];

pub const SOUNDS: &[(&str, &str)] = &[
    ("thock", "Thock"), ("click", "Click"), ("tock", "Tock"),
    ("clack", "Clack"), ("typewriter", "Typewriter"), ("snap", "Snap"),
    ("wood", "Wood block"), ("pop", "Pop"), ("boop", "Boop"),
    ("blip", "Blip"), ("coin", "Coin"), ("pew", "Pew"),
    ("squeak", "Squeak"), ("quack", "Quack"), ("chirp", "Chirp"),
];

pub const INTERACT_KEYS: &[&str] = &["option", "command", "control", "off"];
pub const FADE_CHOICES: &[i64] = &[3, 6, 10, 0];

pub fn style(id: &str) -> Option<&'static Style> {
    STYLES.iter().find(|s| s.id == id)
}

pub fn style_title(id: &str) -> &'static str {
    style(id).map(|s| s.title).unwrap_or("Unknown")
}

pub fn migrate_style(id: &str) -> &str {
    match id {
        "dot" => "laser",
        "sunny" => "coral",
        other => other,
    }
}

pub fn hex_rgb(value: &str) -> (f64, f64, f64) {
    let s = value.trim_start_matches('#');
    let parse = |a: usize| -> f64 {
        u8::from_str_radix(&s[a..a + 2], 16).unwrap_or(0) as f64 / 255.0
    };
    (parse(0), parse(2), parse(4))
}

pub fn resolved_ink(choice: &str, style_id: &str) -> (f64, f64, f64) {
    let colour = INK_CHOICES
        .iter()
        .find(|(id, _, _)| *id == choice)
        .and_then(|(_, _, colour)| *colour)
        .unwrap_or_else(|| style(style_id).or_else(|| STYLES.first()).unwrap().ink);
    hex_rgb(colour)
}

pub fn is_valid_style(id: &str) -> bool {
    style(id).is_some()
}
