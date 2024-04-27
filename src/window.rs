use super::pixels;
use serde::{Deserialize, Serialize};
use std::ops::Not;

pub struct Tracker {
    pub placement: Placement,
    pub last_size: pixels::Size,
}

impl Tracker {
    pub fn new(is_fullscreen: bool, window_size: pixels::Size) -> Tracker {
        Tracker {
            placement: Placement::new(is_fullscreen),
            last_size: window_size,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Default)]
pub enum Placement {
    Fullscreen,
    #[default]
    Windowed,
}

impl Placement {
    pub fn new(is_fullscreen: bool) -> Placement {
        if is_fullscreen {
            Placement::Fullscreen
        } else {
            Placement::Windowed
        }
    }

    pub fn is_fullscreen(self) -> bool {
        match self {
            Placement::Fullscreen => true,
            Placement::Windowed => false,
        }
    }
}

impl Not for Placement {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Placement::Fullscreen => Placement::Windowed,
            Placement::Windowed => Placement::Fullscreen,
        }
    }
}
