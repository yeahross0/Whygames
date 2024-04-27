use serde::{Deserialize, Serialize};
use strum_macros::EnumString;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Sprite {
    pub index: u32,
    pub size: SpriteSize,
}

impl Sprite {
    pub fn none() -> Sprite {
        Sprite {
            index: 0,
            size: SpriteSize::Empty,
        }
    }
}

use strum_macros::Display;
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default, EnumString, Display,
)]
pub enum SpriteSize {
    #[default]
    Empty,
    Square(u32),
    InnerBg,
    OuterBg,
}
