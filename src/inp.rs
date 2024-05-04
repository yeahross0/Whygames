use std::collections::HashMap;

use crate::{drawer::Camera, play::position_in_world};

use super::pixels;
use macroquad::input::KeyCode;
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, EnumString)]
pub enum Button {
    #[default]
    Up,
    Down,
    Press,
    Release,
}

impl Button {
    pub fn is_down(self) -> bool {
        self == Button::Press || self == Button::Down
    }

    pub fn is_up(self) -> bool {
        self == Button::Release || self == Button::Up
    }

    pub fn is_pressed(self) -> bool {
        self == Button::Press
    }

    pub fn is_released(self) -> bool {
        self == Button::Release
    }

    #[allow(dead_code)]
    pub fn is_held_down(self) -> bool {
        self == Button::Down
    }

    #[allow(dead_code)]
    pub fn is_left_up(self) -> bool {
        self == Button::Up
    }
}

pub fn updated_button(button: Button, is_down: bool) -> Button {
    match button {
        Button::Up | Button::Release => {
            if is_down {
                Button::Press
            } else {
                Button::Up
            }
        }
        Button::Down | Button::Press => {
            if is_down {
                Button::Down
            } else {
                Button::Release
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RepeatableButton {
    pub button: Button,
    pub is_repeated: bool,
    repeat_count: i32,
}

// TODO: Tidy
impl RepeatableButton {
    pub fn update(&mut self, is_down: bool) {
        self.button = updated_button(self.button, is_down);
        self.is_repeated = false;
        if self.button.is_pressed() {
            self.is_repeated = true;
            self.repeat_count = 20;
        }
        if self.button.is_down() {
            if self.repeat_count > 0 {
                self.repeat_count -= 1;
            }
        } else {
            self.repeat_count = 20;
        }
        if self.repeat_count == 1 {
            self.is_repeated = true;
            self.repeat_count = 10;
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Mouse {
    pub position: pixels::Position,
    pub drag: pixels::Position,
    pub left_button: Button,
    pub middle_button: Button,
    pub right_button: Button,
}

impl Mouse {
    pub fn update(
        &mut self,
        position: pixels::Position,
        is_left_down: bool,
        is_middle_down: bool,
        is_right_down: bool,
    ) {
        *self = Mouse {
            position,
            drag: position - self.position,
            left_button: updated_button(self.left_button, is_left_down),
            middle_button: updated_button(self.middle_button, is_middle_down),
            right_button: updated_button(self.right_button, is_right_down),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Input {
    pub outer: Mouse,
    pub inner: Mouse,
    pub rmb_held_down_for: i32,
    pub chars_pressed: Vec<char>,
    pub mouse_scroll: f32,
    pub keyboard: HashMap<KeyCode, RepeatableButton>,
}

impl Input {
    pub fn update(&mut self, inner_camera: Camera, temp_save: &mut bool) {
        self.chars_pressed = pressed_chars();
        self.keyboard
            .get_mut(&KeyCode::Z)
            .unwrap()
            .update(macroquad::input::is_key_down(KeyCode::Z));

        self.keyboard
            .get_mut(&KeyCode::Y)
            .unwrap()
            .update(macroquad::input::is_key_down(KeyCode::Y));

        let mouse_position = {
            let (x, y) = macroquad::input::mouse_position();
            pixels::Position::new(x as i32, y as i32)
        };
        let outer_position = position_in_world(mouse_position, Camera::Outer);
        let inner_position = position_in_world(mouse_position, inner_camera);
        let mut is_left_down =
            macroquad::input::is_mouse_button_down(macroquad::input::MouseButton::Left);
        let is_middle_down =
            macroquad::input::is_mouse_button_down(macroquad::input::MouseButton::Middle);
        let is_right_down =
            macroquad::input::is_mouse_button_down(macroquad::input::MouseButton::Right);

        /*  TODO: */
        if *temp_save
            && !macroquad::input::is_mouse_button_pressed(macroquad::input::MouseButton::Left)
        {
            is_left_down = false;
        } else {
            *temp_save = false;
        }

        self.outer
            .update(outer_position, is_left_down, is_middle_down, is_right_down);
        self.inner
            .update(inner_position, is_left_down, is_middle_down, is_right_down);

        if is_right_down {
            self.rmb_held_down_for += 1;
        } else {
            self.rmb_held_down_for = 0;
        }
    }
}

pub const BACKSPACE_CODE: u32 = 8;
pub const CTRL_Z_CHAR: char = '\u{1a}';
pub const CTRL_Y_CHAR: char = '\u{19}';
pub const FIRST_LEGIT_KEY: u32 = 32;

pub fn pressed_chars() -> Vec<char> {
    let mut chars_pressed = Vec::new();
    while let Some(ch) = macroquad::prelude::get_char_pressed() {
        let ch = workaround_character_issue(ch);

        chars_pressed.push(ch);
    }
    chars_pressed
}

fn workaround_character_issue(ch: char) -> char {
    let should_use_alt_character = macroquad::prelude::is_key_down(KeyCode::LeftShift);
    if should_use_alt_character {
        alternative_character(ch)
    } else {
        ch
    }
}

// Fixes something on web?
fn alternative_character(ch: char) -> char {
    match ch {
        '\'' => '@',
        '/' => '?',
        _ => ch,
    }
}
