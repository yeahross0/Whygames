use std::collections::HashMap;
use std::str::FromStr;

pub use super::play::BitmapFont;

pub use super::play::Member;

use super::anim::AnimationStyle;
use super::art::{Sprite, SpriteSize};
use super::colours;
use super::drawer::{sprite_size_in_pixels, FancyText};
use super::history;
use super::inp::Button;
use super::pixels;
use super::play;
use super::play::Text;
use super::serial;
use super::serial::{
    CollisionWith, Demand, Direction, Hover, JumpLocation, Motion, Question, Switch, When,
    WhichButton, WinStatus,
};
use macroquad::{color::Color as Colour, math::Vec2, texture::Texture2D};
use regex::Regex;
use std::fmt::Write;

const MAX_EDITABLE_COUNT: usize = 32;

#[derive(Debug, Default, Clone)]
pub struct Editor {
    pub selected_index: usize,
    pub page: isize,
    pub edit_text_index: Option<usize>,
    pub choices: AssetChoices,
    pub animation: Vec<Sprite>,
    pub index_tracker: usize,
    pub previous_hovered_indices: Vec<usize>,
    pub original_position: Vec2,
    pub original_mouse_position: pixels::Position,
    pub undo_stack: Vec<history::Step>,
    pub redo_stack: Vec<history::Step>,
    pub inner_copy: Option<play::Game>,
    pub paused_copy: Option<play::Game>,
}

#[derive(Debug, Clone, Default)]
pub struct AssetChoices {
    pub games: Vec<String>,
    pub images: Vec<ImageChoice>,
    pub music: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImageChoice {
    pub name: String,
    pub texture: Option<Texture2D>,
}

// TODO: Correct locaiton for this fn, might notj ust be editor stuff
// TODO: Replacing ' ' with "" might hurt me at some point
pub fn get_typed_variable<T: FromStr>(
    context_variables: &HashMap<String, String>,
    name: &str,
) -> Option<T> {
    context_variables
        .get(name)
        .map(|s| s.replace(' ', ""))
        .and_then(|s| T::from_str(&s).ok())
}

// TODO: Think about errors
pub fn index_from_context(context_variables: &HashMap<String, String>, name: &str) -> usize {
    get_typed_variable::<usize>(context_variables, name).unwrap_or(1) - 1
}

pub fn chore_index_from_context(context_variables: &HashMap<String, String>) -> usize {
    index_from_context(context_variables, "Chore Index")
}

pub fn question_index_from_context(context_variables: &HashMap<String, String>) -> usize {
    index_from_context(context_variables, "Question Index")
}

pub fn demand_index_from_context(context_variables: &HashMap<String, String>) -> usize {
    index_from_context(context_variables, "Demand Index")
}

pub fn sprite_from_context(context_variables: &HashMap<String, String>) -> Sprite {
    let mut sprite_size = get_typed_variable(context_variables, "Sprite Type").unwrap();

    if let SpriteSize::Square(n) = &mut sprite_size {
        *n = u32::from_str(context_variables.get("Sprite Size").unwrap()).unwrap();
    }

    let sprite_index = get_typed_variable(context_variables, "Sprite Index").unwrap();

    Sprite {
        index: sprite_index,
        size: sprite_size,
    }
}

pub fn index_from_member_text(re: &Regex, text: &str) -> Option<usize> {
    if let Some(cap) = re.captures(text) {
        if let Ok(i) = cap[1].parse::<usize>() {
            return Some(i);
        }
    }
    None
}

pub fn question_from_context(context_variables: &HashMap<String, String>) -> Question {
    let question_str = context_variables.get("Question").unwrap().replace(' ', "");

    let mut question = Question::from_str(&question_str).unwrap();

    match &mut question {
        Question::IsMouseInteracting {
            which: _which,
            state,
            hover,
        } => {
            *state = context_variables
                .get("Button")
                .map(|s| s.replace(' ', ""))
                .and_then(|s| Button::from_str(&s).ok());
            *hover = Hover::from_str(context_variables.get("Hover").unwrap()).unwrap();
        }
        Question::IsTimeAt(when) => {
            if let Some(w) = context_variables
                .get("When")
                .map(|s| s.replace(' ', ""))
                .and_then(|s| When::from_str(&s).ok())
            {
                *when = w;
                match when {
                    When::Exact { time } => {
                        *time = get_typed_variable(context_variables, "Time").unwrap();
                    }
                    When::Random { start, end } => {
                        let s: usize = get_typed_variable(context_variables, "Time").unwrap();
                        let e = get_typed_variable(context_variables, "End Time").unwrap();
                        let (s, e) = (s.min(e), s.max(e));
                        *start = s;
                        *end = e;
                    }
                    _ => {}
                }
            }
        }
        Question::IsCollidingWith(collision_with) => {
            if let Some(cw) = context_variables
                .get("Collision With")
                .map(|s| s.replace(' ', ""))
                .and_then(|s| CollisionWith::from_str(&s).ok())
            {
                *collision_with = cw;
                match collision_with {
                    CollisionWith::Area(area) => {
                        let min_x = context_variables["MinX"].parse().unwrap_or(0);
                        let min_y = context_variables["MinY"].parse().unwrap_or(0);
                        let max_x = context_variables["MaxX"].parse().unwrap_or(0);
                        let max_y = context_variables["MaxY"].parse().unwrap_or(0);

                        *area = pixels::Rect::aabb(
                            min_x.min(max_x),
                            min_y.min(max_y),
                            min_x.max(max_x),
                            min_y.max(max_y),
                        );
                    }
                    CollisionWith::Member { name } => {
                        if let Some(member_name) = context_variables.get("Member Name") {
                            *name = member_name.to_owned();
                        }
                    }
                }
            }
        }
        Question::IsWinStatusSetTo(win_status) => {
            if let Some(w) = context_variables
                .get("Win Status")
                .map(|s| s.replace(' ', ""))
                .and_then(|s| WinStatus::from_str(&s).ok())
            {
                *win_status = w;
            }
        }
        Question::IsSwitchSetTo { name, switch } => {
            if let Some(sw) = context_variables
                .get("Switch State")
                .map(|s| s.replace(' ', ""))
                .and_then(|s| Switch::from_str(&s).ok())
            {
                *switch = sw;
            }
            if let Some(member_name) = context_variables.get("Member Name") {
                *name = member_name.to_owned();
            }
        }
        Question::IsSpriteSetTo(sprite) => {
            *sprite = sprite_from_context(context_variables);
        }
        Question::IsVariableSetTo { name, value } => {
            *name = context_variables.get("Key").cloned().unwrap_or_default();
            *value = context_variables.get("Text").cloned().unwrap_or_default();
        }
        Question::IsPagedVariableSelected { name, value } => {
            *name = context_variables.get("Key").cloned().unwrap_or_default();
            *value = context_variables.get("Text").cloned().unwrap_or_default();
        }
        Question::IsPagedVariableValid { name, value } => {
            *name = context_variables.get("Key").cloned().unwrap_or_default();
            *value = context_variables.get("Text").cloned().unwrap_or_default();
        }
        Question::IsTextSetTo { value } => {
            *value = context_variables.get("Text").cloned().unwrap_or_default();
        }
        Question::IsShortcutUsed(shortcut) => {
            *shortcut = get_typed_variable(context_variables, "Shortcut").unwrap_or_default();
        }
        _ => {}
    }
    question
}

pub fn demand_from_context(
    context_variables: &HashMap<String, String>,
    animation_scratch: &[Sprite],
) -> Demand {
    let demand_str = context_variables.get("Demand").unwrap().replace(' ', "");

    let mut demand = Demand::from_str(&demand_str).unwrap();

    match &mut demand {
        Demand::SetSwitch(switch) => {
            if let Some(sw) = context_variables
                .get("Switch")
                .map(|s| s.replace(' ', ""))
                .and_then(|s| Switch::from_str(&s).ok())
            {
                *switch = sw;
            }
        }
        Demand::SetSprite(sprite) => {
            *sprite = sprite_from_context(context_variables);
        }
        Demand::SetText(text) => {
            text.contents = context_variables.get("Text").cloned().unwrap_or_default();
            // TODO:
            text.colour = serial::Colour {
                r: 0.973,
                g: 0.965,
                b: 0.957,
                a: 1.0,
            };
        }
        Demand::Motion(motion) => {
            if let Some(m) = get_typed_variable(context_variables, "Motion") {
                *motion = m;
            }

            match motion {
                Motion::JumpTo(jump_location) => {
                    if let Some(jl) = context_variables
                        .get("Jump Location")
                        .map(|s| s.replace(' ', ""))
                        .and_then(|s| JumpLocation::from_str(&s).ok())
                    {
                        *jump_location = jl;

                        match jump_location {
                            JumpLocation::Point(position) => {
                                position.x =
                                    get_typed_variable(context_variables, "X").unwrap_or_default();
                                position.y =
                                    get_typed_variable(context_variables, "Y").unwrap_or_default();
                            }
                            JumpLocation::Area(area) => {
                                let min_x = context_variables["MinX"].parse().unwrap_or(0);
                                let min_y = context_variables["MinY"].parse().unwrap_or(0);
                                let max_x = context_variables["MaxX"].parse().unwrap_or(0);
                                let max_y = context_variables["MaxY"].parse().unwrap_or(0);

                                *area = pixels::Rect::aabb(
                                    min_x.min(max_x),
                                    min_y.min(max_y),
                                    min_x.max(max_x),
                                    min_y.max(max_y),
                                );
                            }
                            JumpLocation::Member { name } => {
                                if let Some(member_name) = context_variables.get("Member Name") {
                                    *name = member_name.to_owned();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Motion::Go { direction, speed } => {
                    if context_variables["North"] == "True" {
                        direction.insert(Direction::North);
                    }
                    if context_variables["North East"] == "True" {
                        direction.insert(Direction::NorthEast);
                    }
                    if context_variables["East"] == "True" {
                        direction.insert(Direction::East);
                    }
                    if context_variables["South East"] == "True" {
                        direction.insert(Direction::SouthEast);
                    }
                    if context_variables["South"] == "True" {
                        direction.insert(Direction::South);
                    }
                    if context_variables["South West"] == "True" {
                        direction.insert(Direction::SouthWest);
                    }
                    if context_variables["West"] == "True" {
                        direction.insert(Direction::West);
                    }
                    if context_variables["North West"] == "True" {
                        direction.insert(Direction::NorthWest);
                    }

                    *speed = get_typed_variable(context_variables, "Speed").unwrap_or_default();
                }
                Motion::ClampPosition { area } => {
                    let min_x = context_variables["MinX"].parse().unwrap_or(0);
                    let min_y = context_variables["MinY"].parse().unwrap_or(0);
                    let max_x = context_variables["MaxX"].parse().unwrap_or(0);
                    let max_y = context_variables["MaxY"].parse().unwrap_or(0);

                    *area = pixels::Rect::aabb(
                        min_x.min(max_x),
                        min_y.min(max_y),
                        min_x.max(max_x),
                        min_y.max(max_y),
                    );
                }
                Motion::Roam {
                    speed,
                    area,
                    roam_type,
                    movement_handling,
                } => {
                    let min_x = context_variables["MinX"].parse().unwrap_or(0);
                    let min_y = context_variables["MinY"].parse().unwrap_or(0);
                    let max_x = context_variables["MaxX"].parse().unwrap_or(0);
                    let max_y = context_variables["MaxY"].parse().unwrap_or(0);

                    *area = pixels::Rect::aabb(
                        min_x.min(max_x),
                        min_y.min(max_y),
                        min_x.max(max_x),
                        min_y.max(max_y),
                    );

                    if let Some(sp) = get_typed_variable(context_variables, "Speed") {
                        *speed = sp;
                    }

                    if let Some(roam) = get_typed_variable(context_variables, "Roam Type") {
                        *roam_type = roam;
                    }

                    if let Some(movement) =
                        get_typed_variable(context_variables, "Movement Handling")
                    {
                        *movement_handling = movement;
                    }
                }
                Motion::Swap { name } => {
                    if let Some(member_name) = context_variables.get("Member Name") {
                        *name = member_name.to_owned();
                    }
                }
                Motion::GoToPoint { point, speed } => {
                    point.x = get_typed_variable(context_variables, "X").unwrap_or_default();
                    point.y = get_typed_variable(context_variables, "Y").unwrap_or_default();
                    *speed = get_typed_variable(context_variables, "Speed").unwrap_or_default();
                }
                Motion::AttachFromPositions { name } => {
                    if let Some(member_name) = context_variables.get("Member Name") {
                        *name = member_name.to_owned();
                    }
                }
                // TODO: Motion::Target
                _ => {}
            }
        }
        Demand::SetVariable { name, value } => {
            *name = context_variables.get("Key").cloned().unwrap_or_default();
            *value = context_variables.get("Text").cloned().unwrap_or_default();
        }
        Demand::SetVariableFromText { name }
        | Demand::SetTextFromVariable { name }
        | Demand::Add1ToVariable { name }
        | Demand::Sub1FromVariable { name } => {
            *name = context_variables.get("Key").cloned().unwrap_or_default();
        }
        Demand::SelectPagedVariable { name, value } => {
            *name = context_variables.get("Key").cloned().unwrap_or_default();
            *value = context_variables.get("Text").cloned().unwrap_or_default();
        }
        Demand::MoveToGame { name } => {
            let game_filename = context_variables.get("Game File Name").unwrap().to_string();
            *name = game_filename;
        }
        Demand::FadeToGame { name } => {
            let game_filename = context_variables.get("Game File Name").unwrap().to_string();
            *name = game_filename;
        }
        Demand::AddToQueue { name } => {
            let game_filename = context_variables.get("Game File Name").unwrap().to_string();
            *name = game_filename;
        }
        Demand::Animate {
            style,
            speed,
            sprites,
        } => {
            *style = get_typed_variable(context_variables, "Animation Style").unwrap_or_default();
            *speed = get_typed_variable(context_variables, "Speed").unwrap_or_default();
            *sprites = animation_scratch.to_vec();
        }
        Demand::PlaySound { name } => {
            *name = context_variables.get("Sound").unwrap().to_string();
        }
        _ => {}
    }
    demand
}

pub fn hovered_in_general_area(
    members: &[play::Member],
    position: pixels::Position,
    font: &play::BitmapFont,
) -> Vec<usize> {
    let mut hovered_indices = Vec::new();
    for (i, member) in members.iter().enumerate() {
        if is_position_in_general_area(position, member, font) {
            hovered_indices.push(i);
        }
    }

    hovered_indices.reverse();
    hovered_indices
}

// Round up to the nearest 8th value
fn eightify(value: u32) -> u32 {
    if value % 8 != 0 {
        value / 8 * 8 + 8
    } else {
        value
    }
}

pub fn general_area_size(member: &play::Member, font: &BitmapFont) -> pixels::Size {
    let mut size = match member.sprite.size {
        SpriteSize::Empty => {
            let width = font.text_width(&member.text.contents).max(4);
            let height = font.char_height;
            pixels::Size::new(width, height)
        }
        _ => sprite_size_in_pixels(member.sprite.size),
    };
    size.w = eightify(size.w);
    size.h = eightify(size.h);

    size
}

// Adjusted for clicking in editor
pub fn is_position_in_general_area(
    position: pixels::Position,
    member: &play::Member,
    font: &play::BitmapFont,
) -> bool {
    let size = general_area_size(member, font);

    let target = pixels::Rect::from_centre(member.position.into(), size);

    target.contains_point(position)
}

pub fn max_var_per_page(members: &[play::Member], var: &str) -> usize {
    let mut buffer = String::with_capacity(32);
    let mut biggest = 0;
    let m_names: Vec<&String> = members
        .iter()
        .map(|m| &m.text.contents)
        .filter(|s| s.starts_with('{') && s.contains(var))
        .collect();
    for j in (1..=MAX_EDITABLE_COUNT).rev() {
        buffer.clear();
        write!(buffer, "{{{} {}}}", var, j).unwrap();
        if m_names.contains(&&buffer) {
            biggest = j;
            break;
        }
    }

    biggest
}

// TODO: Look where this is used in main.rs
// should probably be higher abstracted fn
pub fn padded_len(len: usize, max_per_page: usize) -> usize {
    if len % max_per_page == 0 {
        len
    } else {
        len + max_per_page
    }
}

pub fn offset_for_page(page: isize, max_per_page: usize, len: usize) -> usize {
    (page * max_per_page as isize).rem_euclid((len / max_per_page * max_per_page) as isize) as usize
}

// TODO: simple_text for :String too
pub fn simple_text(text: &str) -> Vec<FancyText> {
    vec![text.plain()]
}

pub trait Fancy {
    fn plain(&self) -> FancyText;

    fn in_colour(&self, colour: Colour) -> FancyText;
}

impl Fancy for &str {
    fn plain(&self) -> FancyText {
        FancyText::Plain {
            text: self.to_string(),
        }
    }

    fn in_colour(&self, colour: Colour) -> FancyText {
        FancyText::InColour {
            text: self.to_string(),
            colour,
        }
    }
}

impl Fancy for String {
    // TODO: Remove to_string()
    fn plain(&self) -> FancyText {
        FancyText::Plain {
            text: self.to_string(),
        }
    }

    fn in_colour(&self, colour: Colour) -> FancyText {
        FancyText::InColour {
            text: self.to_string(),
            colour,
        }
    }
}

fn shorten(s: &str, limit: usize) -> String {
    let real_limit = limit.max(3).min(s.len());
    let mut out = s[..real_limit].to_string();
    if out.len() < s.len() {
        out.push_str("...")
    }
    out
}

trait ToText {
    fn to_text(&self) -> Text;

    fn to_white_text(&self) -> Text;
}

impl ToText for &str {
    fn to_text(&self) -> Text {
        Text::from_str(self)
    }

    fn to_white_text(&self) -> Text {
        Text::new(self.to_string(), colours::WHITE)
    }
}

pub fn fancy_question_text_for_list(
    member: &play::Member,
    subgame: &play::Game,
    editor: &Editor,
    context_variables: &HashMap<String, String>,
) -> Vec<FancyText> {
    let chore_index = chore_index_from_context(context_variables);

    for i in 0..serial::QUESTION_COUNT {
        if member.text.contents == format!("{{Question {}}}", i + 1) {
            let question =
                &subgame.members[editor.selected_index].todo_list[chore_index].questions[i];

            return match question {
                Question::None => simple_text("New"),
                question => {
                    let mut fancy_text = fancy_question_text(question);
                    fancy_text.push("?".plain());
                    fancy_text
                }
            };
        }
    }

    Vec::new()
}

pub fn fancy_question_text_for_chore(
    member: &play::Member,
    subgame: &play::Game,
    editor: &Editor,
) -> Vec<FancyText> {
    for i in 0..serial::CHORE_COUNT {
        // TODO: Reconfigure to remove clone
        if member.text.contents == format!("{{Chore {}}}", i + 1) {
            let question = &subgame.members[editor.selected_index].todo_list[i].questions[0];

            let mut fancy_text = match &question {
                Question::None => {
                    if subgame.members[editor.selected_index].todo_list[i].questions
                        == [
                            Question::None,
                            Question::None,
                            Question::None,
                            Question::None,
                            Question::None,
                            Question::None,
                        ]
                        && subgame.members[editor.selected_index].todo_list[i].demands
                            == [
                                Demand::None,
                                Demand::None,
                                Demand::None,
                                Demand::None,
                                Demand::None,
                                Demand::None,
                            ]
                    {
                        simple_text("New")
                    } else if subgame.members[editor.selected_index].todo_list[i].questions
                        == [
                            Question::None,
                            Question::None,
                            Question::None,
                            Question::None,
                            Question::None,
                            Question::None,
                        ]
                    {
                        simple_text("Every Frame")
                    } else {
                        simple_text("...")
                    }
                }
                question => fancy_question_text(question),
            };

            if *question != Question::None {
                fancy_text.push("...".plain())
            }

            return fancy_text;
        }
    }

    Vec::new()
}

pub fn fancy_question_text(question: &Question) -> Vec<FancyText> {
    match question {
        Question::None => simple_text("None"),
        Question::IsTimeAt(When::Start) => {
            vec![
                "Has the game ".plain(),
                "just started".in_colour(colours::GREEN),
            ]
        }
        Question::IsTimeAt(When::End) => {
            vec!["Is the game ".plain(), "ending".in_colour(colours::GREEN)]
        }
        Question::IsTimeAt(When::Exact { time }) => {
            const TIME_DIVISIONS: usize = 12;
            vec![
                "Is the time ".plain(),
                format!(
                    "{}-{}",
                    time / TIME_DIVISIONS + 1,
                    time % TIME_DIVISIONS + 1
                )
                .in_colour(colours::GREEN),
            ]
        }
        Question::IsTimeAt(When::Random { start, end }) => {
            const TIME_DIVISIONS: usize = 12;
            vec![
                "Is randomly in ".plain(),
                format!(
                    "{}-{}",
                    start / TIME_DIVISIONS + 1,
                    start % TIME_DIVISIONS + 1
                )
                .in_colour(colours::GREEN),
                " to ".plain(),
                format!("{}-{}", end / TIME_DIVISIONS + 1, end % TIME_DIVISIONS + 1)
                    .in_colour(colours::GREEN),
            ]
        }
        // Anywhere
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: None,
            hover: Hover::Anywhere,
        } => simple_text("Is the mouse anywhere"),
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Up),
            hover: Hover::Anywhere,
        } => simple_text("Is the screen not clicked"),
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Press),
            hover: Hover::Anywhere,
        } => simple_text("Is the screen clicked"),
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Down),
            hover: Hover::Anywhere,
        } => simple_text("Is the screen held down"),
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Release),
            hover: Hover::Anywhere,
        } => {
            vec![
                "Is the ".plain(),
                "mouse ".in_colour(colours::GREEN),
                "released".in_colour(colours::BLUE),
            ]
        }
        // This
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: None,
            hover: Hover::This,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "hovered".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Up),
            hover: Hover::This,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "left ".plain(),
                "up".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Press),
            hover: Hover::This,
        } => {
            vec![
                "Has ".plain(),
                "this".in_colour(colours::NULLPURPLE),
                " been ".plain(),
                "clicked".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Down),
            hover: Hover::This,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "held ".plain(),
                "down".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Release),
            hover: Hover::This,
        } => {
            vec![
                "Has ".plain(),
                "this".in_colour(colours::NULLPURPLE),
                " been ".plain(),
                "released".in_colour(colours::GREEN),
            ]
        }
        // Top Member
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: None,
            hover: Hover::TopMember,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "top and ".plain(),
                "hovered".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Up),
            hover: Hover::TopMember,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "top and left ".plain(),
                "up".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Press),
            hover: Hover::TopMember,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "top and ".plain(),
                "clicked".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Down),
            hover: Hover::TopMember,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "top and held ".plain(),
                "down".in_colour(colours::GREEN),
            ]
        }
        Question::IsMouseInteracting {
            which: WhichButton::Left,
            state: Some(Button::Release),
            hover: Hover::TopMember,
        } => {
            vec![
                "Is ".plain(),
                "this ".in_colour(colours::NULLPURPLE),
                "top and ".plain(),
                "released".in_colour(colours::GREEN),
            ]
        }

        Question::IsMouseInteracting { which: _which, .. } => {
            simple_text("Is other mouse interaction")
            //todo!();
        }
        Question::IsWinStatusSetTo(WinStatus::Won) => {
            vec![
                "Has the game been ".plain(),
                "won".in_colour(colours::GREEN),
            ]
        }
        Question::IsWinStatusSetTo(WinStatus::Lost) => {
            vec![
                "Has the game been ".plain(),
                "lost".in_colour(colours::GREEN),
            ]
        }
        Question::IsWinStatusSetTo(WinStatus::JustWon) => {
            vec![
                "Has the game ".plain(),
                "just been won".in_colour(colours::GREEN),
            ]
        }
        Question::IsWinStatusSetTo(WinStatus::JustLost) => {
            vec![
                "Has the game ".plain(),
                "just been lost".in_colour(colours::GREEN),
            ]
        }
        Question::IsWinStatusSetTo(WinStatus::NotYetWon) => {
            vec![
                "Has the game not been ".plain(),
                "won".in_colour(colours::GREEN),
                " yet".plain(),
            ]
        }
        Question::IsWinStatusSetTo(WinStatus::NotYetLost) => {
            vec![
                "Has the game not been ".plain(),
                "lost".in_colour(colours::GREEN),
                " yet".plain(),
            ]
        }
        Question::IsSpriteSetTo(sprite) => {
            vec![
                "Is ".plain(),
                "this".in_colour(colours::NULLPURPLE),
                " sprite set to ".plain(),
                FancyText::Sprite(*sprite),
            ]
        }
        Question::IsVariableSetTo { name, value } => {
            vec![
                "Is ".plain(),
                name.in_colour(colours::RED),
                " set to ".plain(),
                value.in_colour(colours::BLUE),
            ]
        }
        Question::IsTextSetTo { value } => {
            vec![
                "Is text".plain(),
                " set to ".plain(),
                value.in_colour(colours::BLUE),
            ]
        }
        Question::IsSwitchSetTo {
            name,
            switch: Switch::On,
        } => {
            vec![
                "Is ".plain(),
                shorten(name, 12).in_colour(colours::RED),
                "'s switch ".plain(),
                "On".in_colour(colours::GREEN),
            ]
        }
        Question::IsSwitchSetTo {
            name,
            switch: Switch::Off,
        } => {
            vec![
                "Is ".plain(),
                shorten(name, 11).in_colour(colours::RED),
                "'s switch ".plain(),
                "Off".in_colour(colours::GREEN),
            ]
        }
        Question::IsSwitchSetTo {
            name,
            switch: Switch::SwitchedOn,
        } => {
            vec![
                "Is ".plain(),
                shorten(name, 11).in_colour(colours::RED),
                " switched on".in_colour(colours::GREEN),
            ]
        }
        Question::IsSwitchSetTo {
            name,
            switch: Switch::SwitchedOff,
        } => {
            vec![
                "Is ".plain(),
                shorten(name, 11).in_colour(colours::RED),
                " switched off".in_colour(colours::GREEN),
            ]
        }
        Question::IsCollidingWith(CollisionWith::Area(area)) => {
            vec!["Has touched ".plain(), FancyText::Area(*area)]
        }
        Question::IsCollidingWith(CollisionWith::Member { name }) => {
            vec![
                "Has touched ".plain(),
                shorten(name, 12).in_colour(colours::RED),
            ]
        }
        Question::IsAnimationFinished => simple_text("Has animation finished"),
        Question::IsPagedVariableValid { name, value } => {
            simple_text(&format!("Is {} {} valid", name, value))
        }
        Question::IsPagedVariableSelected { name, value } => {
            simple_text(&format!("Is {} {} selected", name, value))
        }
        Question::IsAnimationSpriteValid { index } => {
            simple_text(&format!("Is animation {} valid", index))
        }
        Question::IsSubgamePlaying => simple_text("Is subgame playing"),
        Question::IsSubgameEnding => simple_text("Is subgame ending"),
        Question::IsShortcutUsed(shortcut) => {
            vec![
                "Is ".plain(),
                format!("{:?}", shortcut).in_colour(colours::GREEN),
                " shortcut used ".plain(),
            ]
        }
    }
}

pub fn fancy_demand_text_for_list(
    member: &play::Member,
    subgame: &play::Game,
    editor: &Editor,
    context_variables: &HashMap<String, String>,
) -> Vec<FancyText> {
    let chore_index = chore_index_from_context(context_variables);
    for i in 0..serial::DEMAND_COUNT {
        if member.text.contents == format!("{{Demand {}}}", i + 1) {
            let demand =
                &subgame.members[editor.selected_index].todo_list[chore_index].demands[i].clone();

            return match demand {
                Demand::None => simple_text("New"),
                demand => {
                    let mut fancy_text = fancy_demand_text(demand);
                    fancy_text.push("!".plain());
                    fancy_text
                }
            };
        }
    }

    Vec::new()
}

// TODO: Dataify this if possible
// TODO: Consistent colours for questions/demands fancy text
pub fn fancy_demand_text(demand: &Demand) -> Vec<FancyText> {
    match demand {
        Demand::None => simple_text("None"),
        Demand::SetSprite(sprite) => {
            vec![
                "Set ".plain(),
                "this".in_colour(colours::NULLPURPLE),
                " sprite to ".plain(),
                FancyText::Sprite(*sprite),
            ]
        }
        Demand::SetSwitch(Switch::On) | Demand::SetSwitch(Switch::SwitchedOn) => {
            vec![
                "Set ".plain(),
                "this".in_colour(colours::NULLPURPLE),
                " switch ".plain(),
                "on".in_colour(colours::BLUE),
            ]
        }
        Demand::SetSwitch(Switch::Off) | Demand::SetSwitch(Switch::SwitchedOff) => {
            vec![
                "Set ".plain(),
                "this".in_colour(colours::NULLPURPLE),
                " switch ".plain(),
                "off".in_colour(colours::BLUE),
            ]
        }
        Demand::SetText(text) => {
            vec![
                "Set text to ".plain(),
                text.contents.in_colour(colours::AMBER),
            ]
        }
        Demand::Win => {
            vec!["Win ".in_colour(colours::BLUE), "this game".plain()]
        }
        Demand::Lose => {
            vec!["Lose ".in_colour(colours::BLUE), "this game".plain()]
        }
        Demand::Animate {
            style,
            speed: _speed,
            sprites: _sprites,
        } => {
            if *style == AnimationStyle::PlayOnce {
                simple_text("Play an animation once")
            } else {
                simple_text("Loop an animation")
            }
        }
        Demand::StopAnimation => simple_text("Stop animating"),
        Demand::PlaySound { name } => {
            vec![
                "Play ".plain(),
                name.in_colour(colours::AMBER),
                " sound".plain(),
            ]
        }
        Demand::StopMusic => simple_text("Stop the music"),
        Demand::StopSounds => simple_text("Stop all sounds"),
        Demand::Motion(Motion::Stop) => {
            vec!["Stop ".in_colour(colours::BLUE), "moving".plain()]
        }
        Demand::Motion(Motion::Go { direction, speed }) => {
            if direction.len() == 1 {
                let direction = direction.iter().next().unwrap();
                vec![
                    "Go ".in_colour(colours::BLUE),
                    format!("{:?} ", direction).plain(),
                    format!("{:?}", speed).in_colour(colours::GREEN),
                ]
            } else {
                vec![
                    "Go ".in_colour(colours::BLUE),
                    format!("{:?} ", speed).in_colour(colours::GREEN),
                    "in a random direction".plain(),
                ]
            }
        }
        Demand::Motion(Motion::GoToPoint { point, speed }) => {
            vec![
                "Go to ".plain(),
                FancyText::Point(*point),
                format!(" {:?}", speed).in_colour(colours::AMBER),
            ]
        }
        Demand::Motion(Motion::JumpTo(JumpLocation::Mouse)) => {
            vec![
                "Jump ".in_colour(colours::BLUE),
                "to the ".plain(),
                "mouse".in_colour(colours::GREEN),
            ]
        }
        Demand::Motion(Motion::JumpTo(JumpLocation::Point(position))) => {
            vec![
                "Jump ".in_colour(colours::BLUE),
                "to ".plain(),
                FancyText::Point(*position),
            ]
        }
        Demand::Motion(Motion::JumpTo(JumpLocation::Area(area))) => {
            vec![
                "Jump ".in_colour(colours::BLUE),
                "to within ".plain(),
                FancyText::Area(*area),
            ]
        }
        Demand::Motion(Motion::JumpTo(JumpLocation::Member { name })) => {
            vec![
                "Jump ".in_colour(colours::BLUE),
                "to ".plain(),
                shorten(name, 16).in_colour(colours::RED),
            ]
        }
        Demand::Motion(Motion::JumpTo(JumpLocation::Relative { offset: _ })) => {
            vec!["Jump ".in_colour(colours::BLUE), "relative".plain()]
        }
        Demand::Motion(Motion::Swap { name }) => {
            vec![
                "Swap".in_colour(colours::BLUE),
                " places with ".plain(),
                shorten(name, 10).in_colour(colours::RED),
            ]
        }
        Demand::Motion(Motion::Roam {
            roam_type,
            area,
            speed: _speed,
            movement_handling: _movement_handling,
        }) => {
            vec![
                format!("{:?}", roam_type).in_colour(colours::BLUE),
                " in ".plain(),
                FancyText::Area(*area),
            ]
        }
        Demand::Motion(Motion::ClampPosition { area }) => {
            vec!["Clamp in ".plain(), FancyText::Area(*area)]
        }
        Demand::Motion(Motion::Target {
            name,
            offset: _,
            speed: _,
        }) => {
            vec![
                "Target ".in_colour(colours::BLUE),
                shorten(name, 16).in_colour(colours::RED),
            ]
        }
        Demand::Motion(Motion::AttachFromPositions { name }) => {
            vec![
                "Attach".in_colour(colours::BLUE),
                " to ".plain(),
                shorten(name, 14).in_colour(colours::RED),
            ]
        }
        Demand::SetAnimationSprite => simple_text("Set the animation sprite"),
        Demand::AddAnimationSprite => simple_text("Add an animation sprite"),
        Demand::RemoveAnimationSprite => simple_text("Remove an animation sprite"),
        Demand::MoveAnimationUp => simple_text("Move animation sprite up"),
        Demand::MoveAnimationDown => simple_text("Move animation sprite down"),
        Demand::New => simple_text("Make a new game"),
        Demand::Load => simple_text("Load a game"),
        Demand::Save => simple_text("Save the game"),
        Demand::EditText => simple_text("Make this text editable"),
        Demand::SetVariable { name, value } => {
            vec![
                "Set ".plain(),
                name.in_colour(colours::RED),
                " to ".plain(),
                value.in_colour(colours::BLUE),
            ]
        }
        Demand::SetVariableFromText { name } => {
            vec!["Set ".plain(), name.in_colour(colours::RED)]
        }
        Demand::SetTextFromVariable { name } => {
            vec!["Set text from ".plain(), name.in_colour(colours::RED)]
        }
        Demand::SetTextFromPosition { axis, scale: _ } => {
            vec![
                "Set text from ".plain(),
                format!("{:?}", axis).in_colour(colours::GREEN),
                " position".plain(),
            ]
        }
        Demand::SelectPagedVariable { name, value } => {
            vec![
                "Select ".plain(),
                name.in_colour(colours::RED),
                " ".plain(),
                value.in_colour(colours::BLUE),
            ]
        }
        Demand::Add1ToVariable { name } => {
            vec!["Plus 1 to ".plain(), name.in_colour(colours::RED)]
        }
        Demand::Sub1FromVariable { name } => {
            vec!["Minus 1 from ".plain(), name.in_colour(colours::RED)]
        }
        Demand::PreviewMusic => simple_text("Preview the music"),
        Demand::PreviousPage => simple_text("Go to previous page"),
        Demand::NextPage => simple_text("Go to next page"),
        Demand::SetImageFile => simple_text("Set the game image"),
        Demand::SetMusicFile => simple_text("Set the game music"),
        Demand::UpdateScratchFromMember => simple_text("Set variables from member values"),
        Demand::UpdateScratchFromQuestion => simple_text("Set variables from question values"),
        Demand::UpdateScratchFromDemand => simple_text("Set variables from demand values"),
        Demand::SwitchMember => simple_text("Switch member"),
        Demand::AddMember => simple_text("Add a member"),
        Demand::RemoveMember => simple_text("Remove the member"),
        Demand::CloneMember => simple_text("Clone the member"),
        Demand::RenameMember => simple_text("Rename the member"),
        Demand::RemoveChore => simple_text("Remove the chore"),
        Demand::MoveChoreUp => simple_text("Move the chore up"),
        Demand::MoveChoreDown => simple_text("Move the chore down"),
        Demand::MoveQuestionUp => simple_text("Move the question up"),
        Demand::MoveQuestionDown => simple_text("Move the question down"),
        Demand::MoveDemandUp => simple_text("Move the demand up"),
        Demand::MoveDemandDown => simple_text("Move the demand down"),
        Demand::UpdateQuestion => simple_text("Update the question"),
        Demand::UpdateDemand => simple_text("Update the demand"),
        Demand::SetStartSprite => simple_text("Set the starting sprite"),
        Demand::Quit => simple_text("Quit the game"),
        Demand::Stop => simple_text("Stop the game"),
        Demand::Play => simple_text("Play the game"),
        Demand::Pause => simple_text("Pause the game"),
        Demand::MoveToGame { name } => {
            vec![
                "Switch to ".plain(),
                name.in_colour(colours::RED),
                " game".plain(),
            ]
        }
        Demand::FadeToGame { name } => {
            vec![
                "Fade to ".plain(),
                name.in_colour(colours::RED),
                " game".plain(),
            ]
        }
        Demand::FadeOut => {
            vec!["Fade out".plain()]
        }
        Demand::BackInQueue => simple_text("Go to previous game in queue"),
        Demand::NextInQueue => simple_text("Go to next game in queue"),
        Demand::AddToQueue { name } => {
            vec![
                "Add ".plain(),
                name.in_colour(colours::RED),
                " to queue".plain(),
            ]
        }
        Demand::ResetQueue => simple_text("Reset the queue"),
        Demand::ClearArt => simple_text("Clear Art"),
        Demand::SaveArt => simple_text("Save Art"),
        Demand::PlayPhrase => simple_text("Play Maker Phrase"),
        Demand::PausePhrase => simple_text("Pause Maker Phrase"),
        Demand::StopPhrase => simple_text("Stop Maker Phrase"),
        Demand::PreviousInstrument => simple_text("Use Previous Instrument"),
        Demand::NextInstrument => simple_text("Use Next Instrument"),
        Demand::PreviousTrack => simple_text("Go To Next Track"),
        Demand::NextTrack => simple_text("Go To Previous Track"),
    }
}
