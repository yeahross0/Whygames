use super::common::Speed;
use super::track::{has_finished_animation, update_frame};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use strum_macros::EnumString;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, EnumString, Default)]
pub enum AnimationStyle {
    #[default]
    Loop,
    PlayOnce,
}

impl AnimationStyle {
    fn should_loop(self) -> bool {
        self == AnimationStyle::Loop
    }
}

impl std::fmt::Display for AnimationStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AnimationStyle::Loop => "Loop",
            AnimationStyle::PlayOnce => "Play Once",
        };
        write!(f, "{}", s)
    }
}

pub fn animation_time_from_speed(speed: Speed) -> usize {
    match speed {
        Speed::VerySlow => 60,
        Speed::Slow => 30,
        Speed::Normal => 15,
        Speed::Fast => 8,
        Speed::VeryFast => 4,
    }
}

pub trait Trackable: Copy + Debug {}
impl<T> Trackable for T where T: Copy + Debug {}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker<T: Trackable> {
    style: AnimationStyle,
    speed: Speed,
    sprites: Vec<T>,
    frames_until_next_change: usize,
    index: usize,
}

impl<T: Trackable> Tracker<T> {
    fn started(sprites: Vec<T>, speed: Speed, style: AnimationStyle) -> Tracker<T> {
        Tracker {
            style,
            speed,
            sprites,
            frames_until_next_change: animation_time_from_speed(speed),
            index: 0,
        }
    }

    fn update(&mut self) -> Option<T> {
        if update_frame(
            &mut self.index,
            &mut self.frames_until_next_change,
            animation_time_from_speed(self.speed),
            self.sprites.len(),
        ) {
            Some(self.sprites[self.index])
        } else {
            None
        }
    }

    fn is_finished(&self) -> bool {
        has_finished_animation(self.index, self.style.should_loop())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Animation<T: Trackable> {
    Animating {
        tracker: Tracker<T>,
    },
    Finished,
    #[default]
    None,
}

impl<T: Trackable> Animation<T> {
    pub fn started(sprites: Vec<T>, speed: Speed, style: AnimationStyle) -> Animation<T> {
        Animation::Animating {
            tracker: Tracker::started(sprites, speed, style),
        }
    }

    pub fn update(&mut self) -> Option<T> {
        let mut new_sprite = match self {
            Animation::None | Animation::Finished => None,
            Animation::Animating { tracker } => tracker.update(),
        };

        match self {
            Animation::None | Animation::Finished => *self = Animation::None,
            Animation::Animating { tracker } => {
                if new_sprite.is_some() && tracker.is_finished() {
                    *self = Animation::Finished;
                    new_sprite = None
                }
            }
        };

        new_sprite
    }
}
