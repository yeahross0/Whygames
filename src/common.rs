use serde::{Deserialize, Serialize};
use strum_macros::EnumString;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum Speed {
    VerySlow,
    Slow,
    #[default]
    Normal,
    Fast,
    VeryFast,
}

impl std::fmt::Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Speed::VerySlow => "Very Slow",
            Speed::Slow => "Slow",
            Speed::Normal => "Normal",
            Speed::Fast => "Fast",
            Speed::VeryFast => "Very Fast",
        };
        write!(f, "{}", s)
    }
}
