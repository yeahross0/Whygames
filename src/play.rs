// TODO: Module boundaries? SHouldn't reference macroquad here?
use super::art::{Sprite, SpriteSize};
use super::colours;
use super::common::Speed;
use super::pixels;
use super::serial::{
    self, AssetFilenames, Axis, Cartridge, CollisionWith, Demand, Direction, Hover, ImageString,
    IntroText, JumpLocation, Motion, MovementHandling, Music, Question, RoamType, SoundString,
    Switch, When, WhichButton, WinStatus, DEMAND_COUNT, QUESTION_COUNT,
};
use super::texture_from_bytes;
use super::Colour;
use super::FilterMode;
use crate::coll::{is_adjusted_subsection_square_active, CollisionObject};
use crate::doodle::DrawTool;
use crate::drawer::{self, sheet_source_rect, sprite_size_in_pixels, Camera};
use crate::edit::{
    chore_index_from_context, demand_from_context, demand_index_from_context, fancy_question_text,
    get_typed_variable, max_var_per_page, offset_for_page, padded_len, question_from_context,
    question_index_from_context, sprite_from_context, Editor, Fancy,
};
use crate::err::WhyResult;
use crate::history::Event;
use crate::inp::{Button, Mouse};
use crate::menu;
use crate::meta::{
    Environment, EDITABLE_SCREEN_NAME, MUSIC_MAKER_NAME, OUTER_CENTRE, PLAY_SCREEN_NAME,
};
use crate::music::{self, MusicMaker};
use crate::nav::{Link, Navigation};
use crate::seeded_rng::SeededRng;
use crate::seeded_rng::{ChooseRandom, RandomRange};
use crate::serial::Shortcut;
use base64::engine::general_purpose::STANDARD_NO_PAD as BaseEncoder;
use base64::Engine;
use macroquad::{
    logging as log,
    math::{Rect, Vec2},
    texture::{Image, Texture2D},
};
use std::collections::{HashMap, HashSet};

pub use super::anim::Animation;
pub use super::serial::GameSize as Size;
pub use super::serial::Length;

#[derive(Clone, Debug)]
pub struct BitmapFont {
    pub texture: Texture2D,
    pub source_rects: Vec<pixels::Rect>,
    pub char_height: u32,
    conversion_map: HashMap<char, usize>,
}

impl BitmapFont {
    pub fn new(texture: Texture2D) -> BitmapFont {
        let mut font_image = texture.get_texture_data();

        let is_key = |colour: Colour| {
            colour.a == 1.0 && colour.r == 1.0 && colour.g == 0.0 && colour.b == 1.0
        };

        let is_font_pixel = |colour: Colour| !is_key(colour);

        let char_spacing = 1;
        let line_spacing = 1;

        let char_height = {
            let mut char_height = 0;

            for y in line_spacing..font_image.height as u32 {
                let colour = font_image.get_pixel(char_spacing, y);
                if is_key(colour) {
                    break;
                }
                char_height += 1;
            }
            char_height
        };

        let font_source_rects = {
            let mut font_source_rects = Vec::new();
            if char_height != 0 {
                for y in (line_spacing..font_image.height as u32)
                    .step_by(char_height + char_spacing as usize)
                {
                    let mut start_x = char_spacing;

                    let mut on = false;

                    for x in 0..font_image.width as u32 {
                        let colour = font_image.get_pixel(x, y);
                        if on == is_font_pixel(colour) {
                            continue;
                        }
                        if on && !is_font_pixel(colour) {
                            on = false;
                            font_source_rects.push(pixels::Rect::aabb(
                                start_x as i32,
                                y as i32,
                                x as i32,
                                y as i32 + char_height as i32,
                            ));
                        }
                        if !on && is_font_pixel(colour) {
                            on = true;
                            start_x = x;
                        }
                    }
                }
            }
            font_source_rects
        };

        for y in 0..font_image.height as u32 {
            for x in 0..font_image.width as u32 {
                let colour = font_image.get_pixel(x, y);
                if is_key(colour) {
                    font_image.set_pixel(x, y, Colour::from_rgba(0, 0, 0, 0));
                }
            }
        }

        let font_texture = {
            let texture = Texture2D::from_image(&font_image);
            texture.set_filter(FilterMode::Nearest);
            texture
        };

        BitmapFont {
            texture: font_texture,
            source_rects: font_source_rects,
            char_height: char_height as u32,
            conversion_map: Self::create_conversion_map(),
        }
    }

    pub fn char_width(&self, ch: char) -> u32 {
        let letter_index = self.index_from_letter(ch);
        self.source_rects[letter_index].width()
    }

    // TODO: Make method of BitmapFont
    pub fn text_width(&self, text: &str) -> u32 {
        let mut total_width = 0;
        for letter in text.chars() {
            total_width += self.char_width(letter) + 1;
        }
        if total_width % 2 != 0 {
            total_width += 1;
        }
        total_width
    }

    pub fn index_from_letter(&self, letter: char) -> usize {
        let index = (letter as u32 - 32) as usize;
        if let Some(index) = self.conversion_map.get(&letter) {
            return *index;
        }

        if index < self.source_rects.len() {
            index
        } else if 95 >= self.source_rects.len() {
            0
        } else {
            95
        }
    }

    fn create_conversion_map() -> HashMap<char, usize> {
        let mut conversion_map: HashMap<char, usize> = HashMap::new();
        let offset = 32;
        conversion_map.insert('À', 96);
        conversion_map.insert('Á', 97);
        conversion_map.insert('Â', 98);
        conversion_map.insert('Ä', 99);
        conversion_map.insert('È', 132 - offset);
        conversion_map.insert('É', 133 - offset);
        conversion_map.insert('Ê', 134 - offset);
        conversion_map.insert('Ë', 135 - offset);
        conversion_map.insert('Ì', 136 - offset);
        conversion_map.insert('Í', 137 - offset);
        conversion_map.insert('Î', 138 - offset);
        conversion_map.insert('Ï', 139 - offset);
        conversion_map.insert('Ò', 140 - offset);
        conversion_map.insert('Ó', 141 - offset);
        conversion_map.insert('Ô', 142 - offset);
        conversion_map.insert('Ö', 143 - offset);
        conversion_map.insert('Œ', 144 - offset);
        conversion_map.insert('Ù', 145 - offset);
        conversion_map.insert('Ú', 146 - offset);
        conversion_map.insert('Û', 147 - offset);
        conversion_map.insert('Ü', 148 - offset);
        conversion_map.insert('Ç', 149 - offset);
        conversion_map.insert('Ñ', 150 - offset);
        conversion_map.insert('à', 151 - offset);
        conversion_map.insert('á', 152 - offset);
        conversion_map.insert('â', 153 - offset);
        conversion_map.insert('ä', 154 - offset);
        conversion_map.insert('è', 155 - offset);
        conversion_map.insert('é', 156 - offset);
        conversion_map.insert('ê', 157 - offset);
        conversion_map.insert('ë', 158 - offset);
        conversion_map.insert('ì', 159 - offset);
        conversion_map.insert('í', 160 - offset);
        conversion_map.insert('î', 161 - offset);
        conversion_map.insert('ï', 162 - offset);
        conversion_map.insert('ò', 163 - offset);
        conversion_map.insert('ó', 164 - offset);
        conversion_map.insert('ô', 165 - offset);
        conversion_map.insert('ö', 166 - offset);
        conversion_map.insert('œ', 167 - offset);
        conversion_map.insert('ù', 168 - offset);
        conversion_map.insert('ú', 169 - offset);
        conversion_map.insert('û', 170 - offset);
        conversion_map.insert('ü', 171 - offset);
        conversion_map.insert('ç', 172 - offset);
        conversion_map.insert('ñ', 173 - offset);
        conversion_map.insert('ß', 174 - offset);
        conversion_map.insert('£', 175 - offset);
        conversion_map.insert('€', 176 - offset);

        return conversion_map;
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum GameMode {
    Play,
    Edit,
    Menu,
}

#[derive(Clone, Debug)]
pub struct Assets {
    pub image: Image,
    pub image_string: ImageString,
    pub font: BitmapFont,
    pub font_string: ImageString,
    pub texture: Texture2D,
    pub music_string: Option<SoundString>,
    pub music_data: Option<Vec<u8>>,
    pub filenames: AssetFilenames,
}

impl Assets {
    pub fn from_strings(
        image_string: ImageString,
        font_string: ImageString,
        music_string: Option<SoundString>,
        filenames: AssetFilenames,
    ) -> Assets {
        let bytes = BaseEncoder.decode(&image_string.0).unwrap();
        //let image_string = ImageString(BaseEncoder.encode(&bytes));
        let texture = texture_from_bytes(&bytes).unwrap();
        let image = texture.get_texture_data();

        let bytes = BaseEncoder.decode(font_string.0).unwrap();
        let font_string = ImageString(BaseEncoder.encode(&bytes));
        let font_texture = texture_from_bytes(&bytes).unwrap();
        let font = BitmapFont::new(font_texture);

        let mut music_data = None;
        if let Some(s) = &music_string {
            music_data = Some(BaseEncoder.decode(&s.0).unwrap());
        }

        let _render_target = macroquad::texture::render_target(512, 512);

        Assets {
            texture,
            image,
            image_string,
            font,
            font_string,
            music_string,
            music_data,
            filenames,
        }
    }

    /*pub async fn load(image_filename: &str, font_filename: &str) -> Assets {
        let bytes = bytes_from_dir("images", &image_filename).await.unwrap();
        let image_string = ImageString(BaseEncoder.encode(&bytes));
        let texture = texture_from_bytes(&bytes).unwrap();
        let image = texture.get_texture_data();

        let bytes = bytes_from_dir("fonts", &font_filename).await.unwrap();
        let font_string = ImageString(BaseEncoder.encode(&bytes));
        let font_texture = texture_from_bytes(&bytes).unwrap();
        let font = BitmapFont::new(font_texture);

        let font_texture = system_texture("pixolletta.png").await.unwrap();
        Assets {
            texture,
            image,
            image_string,
            font,
            font_string,
        }
    }*/
}

#[derive(Clone, Debug)]
pub struct Game {
    pub members: Vec<Member>,
    pub assets: Assets,
    pub size: Size,
    pub length: Length,
    pub win_status: WinStatus,
    pub triggered_questions: HashSet<QuestionId>,
    pub frame_number: usize,
    pub intro_text: IntroText,
    pub rng: SeededRng,
}

impl Game {
    pub async fn load(link: &Link) -> WhyResult<Game> {
        let cartridge = Cartridge::load(link).await?;

        let rng = SeededRng::new(macroquad::miniquad::date::now() as u64);

        Ok(game_from_cartridge(cartridge, rng))
    }

    pub fn screen_position(&self) -> pixels::Position {
        self.members
            .iter()
            .find(|member| {
                member.text.contents == PLAY_SCREEN_NAME
                    || member.text.contents == EDITABLE_SCREEN_NAME
            })
            .map(|member| member.position.into())
            .unwrap_or(OUTER_CENTRE)
    }

    // TODO: ?
    pub fn screen_position_option(&self) -> Option<pixels::Position> {
        self.members
            .iter()
            .find(|member| {
                member.text.contents == PLAY_SCREEN_NAME
                    || member.text.contents == EDITABLE_SCREEN_NAME
            })
            .map(|member| member.position.into())
    }

    pub fn music_maker_member(&self) -> Option<&Member> {
        self.members
            .iter()
            .find(|m| m.text.contents == MUSIC_MAKER_NAME)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Member {
    pub name: String,
    pub position: Vec2,
    pub switch: Switch,
    pub applied_switch: Switch,
    pub sprite: Sprite,
    pub motion: ActiveMotion,
    pub animation: Animation<Sprite>,
    pub text: Text,
    pub todo_list: Vec<Chore>,
}

fn get_member<'a>(members: &'a [Member], name: &str) -> &'a Member {
    members.iter().find(|member| member.name == name).unwrap()
}

#[derive(Clone, Debug, Default)]
pub struct Chore {
    pub questions: [Question; QUESTION_COUNT],
    pub demands: [Demand; DEMAND_COUNT],
}

pub fn default_todo_list() -> Vec<Chore> {
    // TODO: Simpler way?
    vec![
        Chore::default(),
        Chore::default(),
        Chore::default(),
        Chore::default(),
        Chore::default(),
        Chore::default(),
    ]
}

#[derive(Clone, Debug, Default)]
pub enum ActiveMotion {
    #[default]
    Stop,
    Go {
        direction: Direction,
        speed: Speed,
    },
    Wiggle {
        area: pixels::Rect,
        speed: Speed,
        handling: MovementHandling,
    },
    Insect {
        area: pixels::Rect,
        speed: Speed,
        handling: MovementHandling,
        velocity: Vec2,
    },
    Reflect {
        area: pixels::Rect,
        speed: Speed,
        handling: MovementHandling,
        velocity: Vec2,
    },
    Bounce {
        area: pixels::Rect,
        speed: Speed,
        handling: MovementHandling,
        velocity: Vec2,
    },
    GoToPoint {
        point: pixels::Position,
        speed: Speed,
    },
    Target {
        name: String,
        offset: Vec2,
        speed: Speed,
    },
    Attach {
        name: String,
        offset: Vec2,
    },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Text {
    pub contents: String,
    pub colour: Colour,
}

impl Text {
    #[allow(dead_code)]
    pub fn blank() -> Text {
        Text::plain("".to_string())
    }

    pub fn new(contents: String, colour: Colour) -> Text {
        Text { contents, colour }
    }

    pub fn plain(contents: String) -> Text {
        Text::new(contents, colours::BLACK)
    }

    pub fn from_str(s: &str) -> Text {
        Text::plain(s.to_owned())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
#[repr(usize)]
pub enum DifficultyLevel {
    #[default]
    Normal = 0,
    Challenge = 1,
    Tough = 2,
}

pub enum SoundQueue {
    Ready { sounds: HashSet<String> },
    Stopped,
}

pub fn update_game(
    game: &mut Game,
    mouse: Mouse,
    sounds_to_play: &mut SoundQueue,
    editor: &mut Editor,
    environment: &mut Environment,
    _navigation: &mut Navigation,
    draw_tool: &mut DrawTool,
    music_maker: &mut MusicMaker,
    subgame: Option<&Game>,
    // TODO: Into some UpdateGameInputStruct
    shortcuts: &HashSet<Shortcut>,
) -> (Vec<Event>, Vec<menu::Action>) {
    let mut events_to_apply = Vec::new();
    let mut menu_actions = Vec::new();

    let mut actions: Vec<Vec<Demand>> = Vec::new();
    for (member_index, member) in game.members.iter().enumerate() {
        let mut requested_demands: Vec<Demand> = Vec::new();

        for (chore_index, chore) in member.todo_list.iter().enumerate() {
            let mut triggered = true;
            for (question_index, question) in chore.questions.iter().enumerate() {
                triggered = triggered
                    && match question {
                        Question::None => true,
                        Question::IsTimeAt(When::Start) => game.frame_number == 0,
                        Question::IsTimeAt(When::End) => match game.length {
                            // TODO: Constants, maybe sub 1?
                            Length::Short => game.frame_number == 240,
                            Length::Long => game.frame_number == 480,
                            Length::Infinite => false,
                        },
                        Question::IsTimeAt(When::Exact { time }) => game.frame_number == *time * 5,
                        Question::IsTimeAt(When::Random { start, end }) => {
                            let question_id =
                                QuestionId::new(member_index, chore_index, question_index);
                            if game.triggered_questions.contains(&question_id) {
                                false
                            } else {
                                let frame_number = game
                                    .rng
                                    .number_in_range((*start).max(game.frame_number), *end);
                                let triggered = game.frame_number == frame_number;
                                if triggered {
                                    game.triggered_questions.insert(question_id);
                                }
                                triggered
                            }
                        }
                        Question::IsMouseInteracting {
                            which,
                            state,
                            hover,
                        } => {
                            let constraint = match hover {
                                Hover::TopMember => game
                                    .members
                                    .iter()
                                    .rev()
                                    .find(|other_member| {
                                        if let SpriteSize::Empty = other_member.sprite.size {
                                            is_position_in_text(
                                                mouse.position,
                                                other_member.position,
                                                &game.assets.font,
                                                &other_member.text.contents,
                                            )
                                        } else {
                                            is_position_in_member_image(
                                                mouse.position,
                                                other_member,
                                                &game.assets.image,
                                            )
                                        }
                                    })
                                    .map(|top| std::ptr::eq(member, top))
                                    .unwrap_or(false),
                                Hover::This => {
                                    if let SpriteSize::Empty = member.sprite.size {
                                        // TODO: For other sources as well? Unoptimiso
                                        let mut width =
                                            game.assets.font.text_width(&member.text.contents);
                                        let height = game.assets.font.char_height;
                                        if let Some(subgame) = subgame {
                                            for i in 0..serial::CHORE_COUNT {
                                                if member.text.contents
                                                    == format!("{{Chore {}}}", i + 1)
                                                {
                                                    let question = &subgame.members
                                                        [editor.selected_index]
                                                        .todo_list[i]
                                                        .questions[0];
                                                    let mut fancy_text =
                                                        fancy_question_text(question);
                                                    fancy_text.push("...".plain());

                                                    let font = &game.assets.font;

                                                    width =
                                                        drawer::fancy_text_width(&fancy_text, font);
                                                }
                                            }
                                        }

                                        let rect = pixels::Rect::from_centre(
                                            member.position.into(),
                                            pixels::Size::new(width, height),
                                        );
                                        rect.contains_point(mouse.position)
                                    } else {
                                        is_position_in_member_image(
                                            mouse.position,
                                            member,
                                            &game.assets.image,
                                        )
                                    }
                                }
                                Hover::Anywhere => true,
                            };

                            let mouse_button = match which {
                                WhichButton::Left => mouse.left_button,
                                WhichButton::Middle => mouse.middle_button,
                                WhichButton::Right => {
                                    mouse.right_button
                                    //unreachable!();
                                }
                            };
                            constraint
                                && match state {
                                    Some(Button::Press) => mouse_button.is_pressed(),
                                    Some(Button::Down) => mouse_button.is_down(),
                                    Some(Button::Up) => mouse_button.is_up(),
                                    Some(Button::Release) => mouse_button.is_released(),
                                    None => true,
                                }
                        }
                        Question::IsSwitchSetTo { name, switch } => {
                            let member_switch = get_member(&game.members, name).switch;
                            if *switch == Switch::Off {
                                member_switch == Switch::SwitchedOff || member_switch == Switch::Off
                            } else if *switch == Switch::On {
                                member_switch == Switch::SwitchedOn || member_switch == Switch::On
                            } else {
                                member_switch == *switch
                            }
                        }
                        Question::IsWinStatusSetTo(win_status) => match win_status {
                            WinStatus::Won => {
                                matches!(game.win_status, WinStatus::Won | WinStatus::JustWon)
                            }
                            WinStatus::Lost => {
                                matches!(game.win_status, WinStatus::Lost | WinStatus::JustLost)
                            }
                            WinStatus::NotYetLost => matches!(
                                game.win_status,
                                WinStatus::NotYetLost
                                    | WinStatus::NotYetWon
                                    | WinStatus::JustWon
                                    | WinStatus::Won
                            ),
                            WinStatus::NotYetWon => matches!(
                                game.win_status,
                                WinStatus::NotYetWon
                                    | WinStatus::NotYetLost
                                    | WinStatus::JustLost
                                    | WinStatus::Lost
                            ),
                            _ => game.win_status == *win_status,
                        },
                        Question::IsSpriteSetTo(sprite) => member.sprite == *sprite,
                        Question::IsAnimationFinished => member.animation == Animation::Finished,
                        Question::IsCollidingWith(CollisionWith::Area(area)) => {
                            // TODO: Optimise
                            let mut was_collision = false;
                            if let SpriteSize::Empty = member.sprite.size {
                                for x in area.min.x..area.max.x {
                                    for y in area.min.y..area.max.y {
                                        let position = pixels::Position::new(x, y);
                                        if is_position_in_text(
                                            position,
                                            member.position,
                                            &game.assets.font,
                                            &member.text.contents,
                                        ) {
                                            was_collision = true;
                                            break;
                                        }
                                    }
                                }
                            } else {
                                for x in area.min.x..area.max.x {
                                    for y in area.min.y..area.max.y {
                                        let position = pixels::Position::new(x, y);
                                        if is_position_in_member_image(
                                            position,
                                            member,
                                            &game.assets.image,
                                        ) {
                                            was_collision = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            was_collision
                        }
                        Question::IsCollidingWith(CollisionWith::Member { name }) => {
                            let maybe = game.members.iter().find(|member| member.name == *name);
                            let other = maybe.unwrap();

                            do_members_collide(member, other, &game.assets.image, &game.assets.font)
                        }
                        Question::IsTextSetTo { value } => member.text.contents == *value,
                        Question::IsVariableSetTo { name, value } => {
                            environment.context.get(name) == Some(value)
                        }
                        Question::IsPagedVariableSelected { name, value } => {
                            let var_index =
                                |value: &str, offset| value.parse().unwrap_or(1) + offset;
                            if name == "Paint" {
                                let max_per_page = max_var_per_page(&game.members, "Paint");

                                let len = padded_len(draw_tool.paint_choices.len(), max_per_page);
                                let offset = offset_for_page(editor.page, max_per_page, len);
                                let index = var_index(value, offset);
                                index == environment.context["Paint Index"].parse().unwrap_or(1)
                            } else if let Some(subgame) = subgame {
                                // TODO: Match case?
                                if name == "Member" {
                                    let max_per_page =
                                        max_var_per_page(&game.members, "Member Preview");

                                    let len = padded_len(subgame.members.len(), max_per_page);
                                    let offset = offset_for_page(editor.page, max_per_page, len);
                                    let index = var_index(value, offset);
                                    index
                                        == environment.context["Member Index"].parse().unwrap_or(1)
                                } else if name == "Image File" {
                                    let max_per_page =
                                        max_var_per_page(&game.members, "Image File");
                                    let len = padded_len(editor.choices.images.len(), max_per_page);
                                    let offset = offset_for_page(editor.page, max_per_page, len);
                                    let index = var_index(value, offset);
                                    index
                                        == environment.context["Image File Index"]
                                            .parse()
                                            .unwrap_or(1)
                                } else if name == "Music File" {
                                    // TODO: Name or File? Is this always consistent?
                                    let max_per_page =
                                        max_var_per_page(&game.members, "Music Name");
                                    let len = padded_len(editor.choices.music.len(), max_per_page);
                                    let offset = offset_for_page(editor.page, max_per_page, len);
                                    let index = var_index(value, offset);
                                    index
                                        == environment.context["Music File Index"]
                                            .parse()
                                            .unwrap_or(1)
                                } else if name == "Game File" {
                                    let max_per_page = max_var_per_page(&game.members, "Game Name");
                                    let len = padded_len(editor.choices.games.len(), max_per_page);
                                    let offset = offset_for_page(editor.page, max_per_page, len);
                                    let index = var_index(value, offset);
                                    index
                                        == environment.context["Game File Index"]
                                            .parse()
                                            .unwrap_or(1)
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                        Question::IsPagedVariableValid { name, value } => {
                            let var_index_sub_1 =
                                |value: &str, offset| value.parse().unwrap_or(1) + offset - 1;
                            // TODO: Different (better?) way of doing things than above?
                            if name == "Member" {
                                if let Some(subgame) = subgame {
                                    let max_per_page =
                                        max_var_per_page(&game.members, "Member Preview");
                                    let len = padded_len(subgame.members.len(), max_per_page);
                                    let offset = offset_for_page(editor.page, max_per_page, len);
                                    let index = var_index_sub_1(value, offset);
                                    index < subgame.members.len()
                                } else {
                                    false
                                }
                            } else if name == "Image File" {
                                let max_per_page = max_var_per_page(&game.members, "Image File");
                                let len = padded_len(editor.choices.images.len(), max_per_page);
                                let offset = offset_for_page(editor.page, max_per_page, len);
                                let index = var_index_sub_1(value, offset);
                                index < editor.choices.images.len()
                            } else if name == "Music File" {
                                let max_per_page = max_var_per_page(&game.members, "Music Name");
                                let len = padded_len(editor.choices.music.len(), max_per_page);
                                let offset = offset_for_page(editor.page, max_per_page, len);
                                let index = var_index_sub_1(value, offset);
                                index < editor.choices.music.len()
                            } else if name == "Game File" {
                                let max_per_page = max_var_per_page(&game.members, "Game Name");
                                let len = padded_len(editor.choices.games.len(), max_per_page);
                                let offset = offset_for_page(editor.page, max_per_page, len);
                                let index = var_index_sub_1(value, offset);
                                index < editor.choices.games.len()
                            } else if name == "Paint" {
                                let max_per_page = max_var_per_page(&game.members, "Paint");
                                let len = padded_len(draw_tool.paint_choices.len(), max_per_page);
                                let offset = offset_for_page(editor.page, max_per_page, len);
                                let index = var_index_sub_1(value, offset);
                                index < draw_tool.paint_choices.len()
                            } else {
                                false
                            }
                        }
                        Question::IsAnimationSpriteValid { index } => {
                            *index <= editor.animation.len()
                        }
                        Question::IsSubgamePlaying => {
                            // TODO: Current way of checking this
                            editor.inner_copy.is_some()
                            //subgame.is_some()
                        }
                        Question::IsSubgameEnding => {
                            //subgame.frame_number ==
                            // TODO: Rnadom number for now
                            game.frame_number == 150
                        }
                        Question::IsShortcutUsed(shortcut) => shortcuts.contains(shortcut),
                    };
            }
            if triggered {
                for demand in &chore.demands {
                    if let Demand::None = demand {
                    } else {
                        requested_demands.push(demand.clone());
                    }
                }
            }
        }
        actions.push(requested_demands);
    }

    game.win_status = match game.win_status {
        WinStatus::JustWon => WinStatus::Won,
        WinStatus::JustLost => WinStatus::Lost,
        w => w,
    };

    for (i, action) in actions.into_iter().enumerate() {
        for demand in action {
            match demand {
                Demand::None => {}
                // Game Stuff
                Demand::SetSprite(sprite) => {
                    game.members[i].sprite = sprite;
                    game.members[i].animation = Animation::None;
                }
                Demand::SetSwitch(switch) => {
                    // TODO: Constrain switch to On/Off
                    game.members[i].applied_switch = match switch {
                        Switch::On | Switch::SwitchedOn => Switch::SwitchedOn,
                        Switch::Off | Switch::SwitchedOff => Switch::SwitchedOff,
                    };
                }
                Demand::SetText(text) => {
                    game.members[i].text = Text {
                        contents: text.contents.to_owned(),
                        colour: Colour {
                            r: text.colour.r,
                            g: text.colour.g,
                            b: text.colour.b,
                            a: text.colour.a,
                        },
                    }
                }
                Demand::Win => {
                    if matches!(
                        game.win_status,
                        WinStatus::NotYetWon | WinStatus::NotYetLost,
                    ) {
                        game.win_status = WinStatus::JustWon;
                    }
                }
                Demand::Lose => {
                    if matches!(
                        game.win_status,
                        WinStatus::NotYetWon | WinStatus::NotYetLost,
                    ) {
                        game.win_status = WinStatus::JustLost;
                    }
                }
                Demand::Animate {
                    style,
                    speed,
                    sprites,
                } => {
                    game.members[i].sprite = sprites[0];
                    game.members[i].animation = Animation::started(sprites.clone(), speed, style);
                }
                Demand::StopAnimation => {
                    game.members[i].animation = Animation::None;
                }
                Demand::Motion(Motion::Stop) => {
                    // TODO: Queueing motions?
                    game.members[i].motion = ActiveMotion::Stop;
                }
                Demand::Motion(Motion::Go { direction, speed }) => {
                    // TODO: Temp clone
                    let directions: Vec<serial::Direction> =
                        direction.clone().into_iter().collect();
                    // TODO: If empty then choose from 8 possible directions
                    let direction = *directions.choose(&mut game.rng).unwrap();
                    game.members[i].motion = ActiveMotion::Go { direction, speed }
                }
                Demand::Motion(Motion::GoToPoint { point, speed }) => {
                    game.members[i].motion = ActiveMotion::GoToPoint { point, speed }
                }
                Demand::Motion(Motion::Swap { name }) => {
                    let maybe_index = game
                        .members
                        .iter()
                        .enumerate()
                        .find(|(_, member)| member.name == *name)
                        .map(|(index, _)| index);
                    if let Some(other_index) = maybe_index {
                        let temp = game.members[other_index].position;
                        game.members[other_index].position = game.members[i].position;
                        game.members[i].position = temp;
                    }
                }
                Demand::Motion(Motion::Roam {
                    roam_type,
                    area,
                    speed,
                    movement_handling,
                }) => {
                    game.members[i].motion = match roam_type {
                        RoamType::Wiggle => ActiveMotion::Wiggle {
                            area,
                            speed,
                            handling: movement_handling,
                        },
                        RoamType::Insect => ActiveMotion::Insect {
                            area,
                            speed,
                            handling: movement_handling,
                            // TODO: Maybe this is ok
                            velocity: Vec2::default(),
                        },
                        RoamType::Reflect => ActiveMotion::Reflect {
                            area,
                            speed,
                            handling: movement_handling,
                            velocity: Vec2::default(),
                        },
                        RoamType::Bounce => ActiveMotion::Bounce {
                            area,
                            speed,
                            handling: movement_handling,
                            velocity: Vec2::default(),
                        },
                    };
                }
                Demand::Motion(Motion::JumpTo(JumpLocation::Point(position))) => {
                    game.members[i].position = position.into();
                }
                Demand::Motion(Motion::JumpTo(JumpLocation::Area(area))) => {
                    // TODO: Handle size of object, (incl bg sizes? what does this mean)
                    log::debug!("AREA BEFORE: {:?}", area);
                    let area = constrained_area(&game.assets.image, &game.members[i], area);
                    log::debug!("AREA AFTER: {:?}", area);
                    if area.w > 0.0 {
                        game.members[i].position.x =
                            game.rng.number_in_range(area.x, area.x + area.w);
                    } else {
                        // TODO: ?
                        game.members[i].position.x = area.x;
                    }
                    if area.h > 0.0 {
                        game.members[i].position.y =
                            game.rng.number_in_range(area.y, area.y + area.h);
                    } else {
                        // TODO: ?
                        game.members[i].position.y = area.y;
                    }
                }
                Demand::Motion(Motion::JumpTo(JumpLocation::Mouse)) => {
                    game.members[i].position = mouse.position.into();
                }
                Demand::Motion(Motion::JumpTo(JumpLocation::Member { name })) => {
                    let maybe = game
                        .members
                        .iter()
                        .find(|member| member.name == *name)
                        .map(|other| other.position);
                    if let Some(other) = maybe {
                        game.members[i].position = other;
                    }
                }
                Demand::Motion(Motion::JumpTo(JumpLocation::Relative { offset })) => {
                    let offset: Vec2 = offset.into();
                    game.members[i].position += offset;
                }
                Demand::Motion(Motion::ClampPosition { area }) => {
                    let constrained_area =
                        constrained_area(&game.assets.image, &game.members[i], area);
                    //let constrained_area = area;
                    game.members[i].position.x = game.members[i]
                        .position
                        .x
                        .min(constrained_area.x + constrained_area.w)
                        .max(constrained_area.x);
                    game.members[i].position.y = game.members[i]
                        .position
                        .y
                        .min(constrained_area.y + constrained_area.h)
                        .max(constrained_area.y);
                }
                Demand::Motion(Motion::Target {
                    name,
                    offset,
                    speed,
                }) => {
                    game.members[i].motion = ActiveMotion::Target {
                        name,
                        offset: offset.into(),
                        speed,
                    };
                }
                Demand::Motion(Motion::AttachFromPositions { name }) => {
                    // TODO: Don't need index
                    let maybe_index = game
                        .members
                        .iter()
                        .enumerate()
                        .find(|(_, member)| member.name == *name)
                        .map(|(index, _)| index);
                    if let Some(other_index) = maybe_index {
                        let offset = game.members[i].position - game.members[other_index].position;
                        game.members[i].motion = ActiveMotion::Attach { name, offset };
                    }
                }
                Demand::PlaySound { name } => match sounds_to_play {
                    SoundQueue::Ready { sounds } => {
                        sounds.insert(name.to_owned());
                    }
                    SoundQueue::Stopped => {}
                },
                Demand::StopMusic => {
                    menu_actions.push(menu::Action::StopMusic);
                }
                // TODO: Terrible way of doing this
                Demand::StopSounds => {
                    *sounds_to_play = SoundQueue::Stopped;
                }
                // Editor Stuff
                Demand::New => {
                    menu_actions.push(menu::Action::New);
                }
                Demand::Load => {
                    menu_actions.push(menu::Action::Load);
                }
                Demand::Save => {
                    menu_actions.push(menu::Action::Save);
                }
                Demand::EditText => {
                    editor.edit_text_index = Some(i);
                }
                Demand::SetVariable { name, value } => {
                    environment.update_var(name, value);
                    log::debug!("CONTEXT: {:?}", environment.context);
                }
                Demand::SetVariableFromText { name } => {
                    environment.update_var(name, game.members[i].text.contents.clone());
                    //log::debug!("CONTEXT: {:?}", environment.context);
                }
                Demand::SetTextFromVariable { name } => {
                    // TODO: Temporary?
                    if !environment.has_valid_var::<u32>("Tempo") {
                        environment.update_var("Tempo", "120");
                    }
                    if !environment.has_valid_var::<u32>("Note Length") {
                        environment.update_var("Note Length", "1");
                    }
                    game.members[i].text.contents =
                        environment.get_var_for_text(&name).unwrap_or_default();
                }
                Demand::SetTextFromPosition { axis, scale } => {
                    let p = match axis {
                        Axis::X => game.members[i].position.x as i32 / scale,
                        Axis::Y => game.members[i].position.y as i32 / scale,
                    };
                    game.members[i].text.contents = p.to_string()
                }
                Demand::SelectPagedVariable { name, value } => {
                    if name == "Paint" {
                        let max_per_page = max_var_per_page(&game.members, "Paint");
                        let len = padded_len(draw_tool.paint_choices.len(), max_per_page);
                        let offset = offset_for_page(editor.page, max_per_page, len);
                        let index = value.parse().unwrap_or(1) + offset;
                        if index - 1 < draw_tool.paint_choices.len() {
                            environment.update_var("Paint Index", index.to_string());
                            draw_tool.tracker.paint_index = index - 1;
                        }
                        // TODO: The subgame distinction?
                    } else if let Some(subgame) = subgame {
                        // IF Name == Member Preview
                        if name == "Member" {
                            let max_per_page = max_var_per_page(&game.members, "Member Preview");
                            let len = padded_len(subgame.members.len(), max_per_page);
                            let offset = offset_for_page(editor.page, max_per_page, len);
                            let index = value.parse().unwrap_or(1) + offset;
                            if index - 1 < subgame.members.len() {
                                environment.update_var("Member Index", index.to_string());
                                environment
                                    .update_var("Member Name", &subgame.members[index - 1].name);
                            }
                        } else if name == "Image File" {
                            let max_per_page = max_var_per_page(&game.members, "Image File");
                            let len = padded_len(editor.choices.images.len(), max_per_page);
                            let offset = offset_for_page(editor.page, max_per_page, len);
                            let index = value.parse().unwrap_or(1) + offset;
                            if index - 1 < editor.choices.images.len() {
                                environment.update_var("Image File Index", index.to_string());
                                environment.update_var(
                                    "Image File Name",
                                    &editor.choices.images[index - 1].name,
                                );
                            }
                        } else if name == "Music File" {
                            let max_per_page = max_var_per_page(&game.members, "Music Name");
                            let len = padded_len(editor.choices.music.len(), max_per_page);
                            let offset = offset_for_page(editor.page, max_per_page, len);
                            let index = value.parse().unwrap_or(1) + offset;
                            if index - 1 < editor.choices.music.len() {
                                environment.update_var("Music File Index", index.to_string());
                                environment.update_var(
                                    "Music File Name",
                                    &editor.choices.music[index - 1],
                                );
                            }
                        } else if name == "Game File" {
                            let max_per_page = max_var_per_page(&game.members, "Game Name");
                            let len = padded_len(editor.choices.games.len(), max_per_page);
                            let offset = offset_for_page(editor.page, max_per_page, len);
                            let index = value.parse().unwrap_or(1) + offset;
                            if index - 1 < editor.choices.games.len() {
                                environment.update_var("Game File Index", index.to_string());
                                environment
                                    .update_var("Game File Name", &editor.choices.games[index - 1]);
                            }
                        }
                    }
                }
                Demand::Add1ToVariable { name } => {
                    if let Some(mut var) = get_typed_variable::<i32>(&environment.context, &name) {
                        var += 1;
                        environment.context.insert(name.to_owned(), var.to_string());
                    }
                }
                Demand::Sub1FromVariable { name } => {
                    if let Some(mut var) = environment.get_typed_var::<i32>(&name) {
                        var -= 1;
                        environment.context.insert(name.to_owned(), var.to_string());
                    }
                }
                Demand::PreviewMusic => {
                    menu_actions.push(menu::Action::PreviewMusic);
                }
                Demand::PreviousPage => {
                    editor.page -= 1;
                }
                Demand::NextPage => {
                    editor.page += 1;
                }
                Demand::UpdateScratchFromMember => {
                    if let Some(subgame) = subgame {
                        environment.update_var(
                            "Sprite Type",
                            subgame.members[editor.selected_index]
                                .sprite
                                .size
                                .to_string(),
                        );
                        if let SpriteSize::Square(n) =
                            subgame.members[editor.selected_index].sprite.size
                        {
                            environment.update_var("Sprite Size", n.to_string());
                        }
                        environment.update_var(
                            "Sprite Index",
                            subgame.members[editor.selected_index]
                                .sprite
                                .index
                                .to_string(),
                        );
                    }
                }
                Demand::UpdateScratchFromQuestion => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let question_index = question_index_from_context(&environment.context);

                    if let Some(subgame) = subgame {
                        match &subgame.members[editor.selected_index].todo_list[chore_index]
                            .questions[question_index]
                        {
                            Question::IsMouseInteracting {
                                which: _which,
                                state,
                                hover,
                            } => {
                                environment.update_var_as_debug("Button", state);
                                environment.update_var_as_debug("Hover", hover);
                            }
                            Question::IsTimeAt(When::Exact { time }) => {
                                environment.update_var("Time", time.to_string());
                            }
                            Question::IsTimeAt(When::Random { start, end }) => {
                                environment.update_var("Time", start.to_string());
                                environment.update_var("End Time", end.to_string());
                            }
                            Question::IsCollidingWith(CollisionWith::Area(area)) => {
                                environment.update_var("MinX", area.min.x.to_string());
                                environment.update_var("MinY", area.min.y.to_string());
                                environment.update_var("MaxX", area.max.x.to_string());
                                environment.update_var("MaxY", area.max.y.to_string());
                            }
                            Question::IsCollidingWith(CollisionWith::Member { name }) => {
                                environment.update_var("Member Name", name);
                            }
                            Question::IsSpriteSetTo(sprite) => {
                                environment.update_var("Sprite Type", sprite.size.to_string());
                                if let SpriteSize::Square(n) = sprite.size {
                                    environment.update_var("Sprite Size", n.to_string());
                                }
                                environment.update_var("Sprite Index", sprite.index.to_string());
                            }
                            Question::IsWinStatusSetTo(win_status) => {
                                environment.update_var_as_debug("Win Status", win_status);
                            }
                            Question::IsSwitchSetTo { name, switch } => {
                                environment.update_var("Member Name", name);
                                environment.update_var_as_debug("Switch", switch);
                            }
                            Question::IsVariableSetTo { name, value }
                            | Question::IsPagedVariableSelected { name, value }
                            | Question::IsPagedVariableValid { name, value } => {
                                environment.update_var("Key", name);
                                environment.update_var("Text", value);
                            }
                            Question::IsTextSetTo { value } => {
                                environment.update_var("Text", value);
                            }
                            _ => {}
                        }
                    }
                }
                Demand::UpdateScratchFromDemand => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let demand_index = demand_index_from_context(&environment.context);

                    if let Some(subgame) = subgame {
                        match &subgame.members[editor.selected_index].todo_list[chore_index].demands
                            [demand_index]
                        {
                            Demand::SetSwitch(switch) => {
                                environment.update_var_as_debug("Switch", switch);
                            }
                            Demand::SetSprite(sprite) => {
                                environment.update_var("Sprite Type", sprite.size.to_string());
                                if let SpriteSize::Square(n) = sprite.size {
                                    environment.update_var("Sprite Size", n.to_string());
                                }
                                environment.update_var("Sprite Index", sprite.index.to_string());
                            }
                            Demand::SetText(text) => {
                                environment.update_var("Text", text.contents.to_string());
                                // TODO: text colour
                            }
                            Demand::Animate {
                                style,
                                speed,
                                sprites,
                            } => {
                                environment.update_var("Animation Index", "1");
                                environment.update_var("Animation Style", style.to_string());
                                environment.update_var("Speed", speed.to_string());

                                editor.animation = sprites.clone();
                            }
                            Demand::Motion(Motion::GoToPoint { point, speed }) => {
                                environment.update_var("X", point.x.to_string());
                                environment.update_var("Y", point.y.to_string());

                                environment.update_var("Speed", speed.to_string());
                            }
                            Demand::Motion(Motion::JumpTo(JumpLocation::Point(point))) => {
                                environment.update_var("X", point.x.to_string());
                                environment.update_var("Y", point.y.to_string());
                            }
                            Demand::Motion(
                                Motion::JumpTo(JumpLocation::Area(area))
                                | Motion::ClampPosition { area },
                            ) => {
                                environment.update_var("MinX", area.min.x.to_string());
                                environment.update_var("MinY", area.min.y.to_string());
                                environment.update_var("MaxX", area.max.x.to_string());
                                environment.update_var("MaxY", area.max.y.to_string());
                            }
                            Demand::Motion(Motion::JumpTo(JumpLocation::Member { name })) => {
                                environment.update_var("Member Name", name);
                            }
                            Demand::Motion(Motion::Go { direction, speed }) => {
                                let repr = |b| {
                                    if b {
                                        "True"
                                    } else {
                                        "False"
                                    }
                                };
                                let dir_conv = [
                                    ("North", Direction::North),
                                    ("North East", Direction::NorthEast),
                                    ("East", Direction::East),
                                    ("South East", Direction::SouthEast),
                                    ("South", Direction::South),
                                    ("South West", Direction::SouthWest),
                                    ("West", Direction::West),
                                    ("North West", Direction::NorthWest),
                                ];
                                for (name, dir) in dir_conv {
                                    environment.update_var(name, repr(direction.contains(&dir)));
                                }
                                environment.update_var("Speed", speed.to_string());
                            }
                            Demand::Motion(Motion::Roam {
                                speed,
                                area,
                                roam_type,
                                movement_handling,
                            }) => {
                                environment.update_var("MinX", area.min.x.to_string());
                                environment.update_var("MinY", area.min.y.to_string());
                                environment.update_var("MaxX", area.max.x.to_string());
                                environment.update_var("MaxY", area.max.y.to_string());

                                environment.update_var("Speed", speed.to_string());

                                environment.update_var_as_debug("Roam Type", roam_type);

                                environment
                                    .update_var_as_debug("Movement Handling", movement_handling);
                            }
                            Demand::Motion(Motion::Swap { name }) => {
                                environment.update_var("Member Name", name);
                            }
                            Demand::SetVariableFromText { name }
                            | Demand::Add1ToVariable { name }
                            | Demand::Sub1FromVariable { name } => {
                                environment.update_var("Key", name);
                            }
                            Demand::SetVariable { name, value }
                            | Demand::SelectPagedVariable { name, value } => {
                                environment.update_var("Key", name);
                                environment.update_var("Text", value);
                            }
                            Demand::MoveToGame { name }
                            | Demand::FadeToGame { name }
                            | Demand::AddToQueue { name } => {
                                environment.update_var("Game File Name", name);
                            }

                            _ => {}
                        }
                    }
                }
                Demand::SwitchMember => {
                    let to_name = &environment.context["Member Name"];
                    if let Some(subgame) = subgame {
                        let maybe_index = subgame
                            .members
                            .iter()
                            .enumerate()
                            .find(|(_, member)| member.name == *to_name)
                            .map(|(index, _)| index);
                        if let Some(other_index) = maybe_index {
                            editor.selected_index = other_index;
                        }
                    }
                }
                // Events
                Demand::AddMember => {
                    let position = Vec2::new(
                        game.rng.number_in_range(0.0, 200.0).floor(),
                        game.rng.number_in_range(0.0, 100.0).floor(),
                    );
                    // TODO: More robust solution
                    let mut valid_numbers: Vec<i32> = Vec::new();
                    for member in &subgame.unwrap().members {
                        if let Ok(num) = member.name.parse() {
                            valid_numbers.push(num);
                        }
                    }
                    valid_numbers.sort();
                    let name = valid_numbers.last().unwrap_or(&0).to_string();
                    events_to_apply.push(Event::AddMember {
                        index: None,
                        member: Member {
                            name,
                            position,
                            text: Text::from_str(""), // "DEBUG"
                            switch: Switch::Off,
                            sprite: Sprite::none(),
                            animation: Animation::None,
                            todo_list: default_todo_list(),
                            ..Default::default()
                        },
                    })
                }
                Demand::RemoveMember => {
                    events_to_apply.push(Event::RemoveMember {
                        index: editor.selected_index,
                    });
                }
                Demand::CloneMember => {
                    if let Some(subgame) = subgame {
                        let mut member = subgame.members[editor.selected_index].clone();
                        // TODO: More robust solution
                        // TODO: Change stuff like CheckSwitch -> Self on Clone
                        // TODO: Tidy up
                        let old_name = &member.name;
                        let splot = old_name.rsplit_once(' ');
                        let tag = splot.map(|sp| sp.0);
                        let number = splot.and_then(|(_, last)| last.parse::<u64>().ok());
                        let new_name = if let (Some(tag), Some(mut number)) = (tag, number) {
                            number += 1;
                            let new_name = format!("{} {}", tag, number);
                            new_name
                        } else {
                            let mut new_name = member.name.clone();
                            new_name.push_str(&game.rng.number_in_range(0, 100).to_string());
                            new_name
                        };
                        rename_in_todo_list(&mut member.todo_list, old_name, &new_name);
                        member.name = new_name;

                        events_to_apply.push(Event::AddMember {
                            index: None,
                            member,
                        })
                    }
                }
                Demand::RenameMember => {
                    // TODO: Is this consistent, to rename to member text?
                    if let Some(subgame) = subgame {
                        events_to_apply.push(Event::RenameMember {
                            index: editor.selected_index,
                            from: subgame.members[editor.selected_index].name.to_owned(),
                            to: game.members[i].text.contents.to_owned(),
                        });
                    }
                }
                Demand::UpdateQuestion => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let question_index = question_index_from_context(&environment.context);

                    let question = question_from_context(&environment.context);

                    events_to_apply.push(Event::UpdateQuestion {
                        id: QuestionId::new(editor.selected_index, chore_index, question_index),
                        question,
                    });
                }
                Demand::RemoveChore => {
                    let chore_index = chore_index_from_context(&environment.context);

                    events_to_apply.push(Event::UpdateChore {
                        id: ChoreId::new(editor.selected_index, chore_index),
                        chore: Box::<Chore>::default(),
                    });
                }
                Demand::MoveChoreUp => {
                    let chore_index = chore_index_from_context(&environment.context);

                    events_to_apply.push(Event::MoveChoreUp {
                        id: ChoreId::new(editor.selected_index, chore_index),
                    });
                }
                Demand::MoveChoreDown => {
                    let chore_index = chore_index_from_context(&environment.context);

                    events_to_apply.push(Event::MoveChoreDown {
                        id: ChoreId::new(editor.selected_index, chore_index),
                    });
                }
                Demand::MoveQuestionUp => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let question_index = question_index_from_context(&environment.context);

                    events_to_apply.push(Event::MoveQuestionUp {
                        id: QuestionId::new(editor.selected_index, chore_index, question_index),
                    });
                }
                Demand::MoveQuestionDown => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let question_index = question_index_from_context(&environment.context);

                    events_to_apply.push(Event::MoveQuestionDown {
                        id: QuestionId::new(editor.selected_index, chore_index, question_index),
                    });
                }
                Demand::MoveDemandUp => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let demand_index = demand_index_from_context(&environment.context);

                    events_to_apply.push(Event::MoveDemandUp {
                        id: DemandId::new(editor.selected_index, chore_index, demand_index),
                    });
                }
                Demand::MoveDemandDown => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let demand_index = demand_index_from_context(&environment.context);

                    events_to_apply.push(Event::MoveDemandDown {
                        id: DemandId::new(editor.selected_index, chore_index, demand_index),
                    });
                }
                Demand::UpdateDemand => {
                    let chore_index = chore_index_from_context(&environment.context);
                    let demand_index = demand_index_from_context(&environment.context);

                    let demand = demand_from_context(&environment.context, &editor.animation);

                    events_to_apply.push(Event::UpdateDemand {
                        id: DemandId::new(editor.selected_index, chore_index, demand_index),
                        demand,
                    });
                }
                Demand::SetStartSprite => {
                    if let Some(subgame) = subgame {
                        let original_sprite = subgame.members[editor.selected_index].sprite;

                        let new_sprite = sprite_from_context(&environment.context);

                        events_to_apply.push(Event::SetStartSprite {
                            index: editor.selected_index,
                            from: original_sprite,
                            to: new_sprite,
                        });
                    }
                }
                Demand::SetAnimationSprite => {
                    if let Some(index) =
                        get_typed_variable::<usize>(&environment.context, "Animation Index")
                    {
                        // TODO: Why was this zero?
                        let index = index - 1;
                        if subgame.is_some() {
                            if editor.animation.is_empty() {
                                editor.animation.push(Sprite::none());
                            }
                            editor.animation[index] = sprite_from_context(&environment.context);
                        }
                    }
                }
                Demand::AddAnimationSprite => {
                    if subgame.is_some() && editor.animation.len() < serial::ANIMATION_SPRITE_COUNT
                    {
                        editor.animation.push(Sprite::default());
                        let index = editor.animation.len();
                        environment.update_var("Animation Index", index.to_string());
                    }
                }
                // TODO: ??????
                Demand::RemoveAnimationSprite => {
                    if let Some(index) = environment.get_typed_var::<usize>("Animation Index") {
                        if index > 0 {
                            let index = index - 1;
                            if subgame.is_some() && !editor.animation.is_empty() {
                                for i in index..(editor.animation.len() - 1) {
                                    editor.animation[i] = editor.animation[i + 1];
                                }

                                if !editor.animation.is_empty() {
                                    editor.animation.remove(editor.animation.len() - 1);
                                }

                                let index = index.max(editor.animation.len());
                                environment.update_var("Animation Index", index.to_string());
                            }
                        }
                    }
                }
                Demand::MoveAnimationUp => {
                    if let Some(index) = environment.get_typed_var::<usize>("Animation Index") {
                        let index = index - 1;
                        if subgame.is_some() && index > 0 {
                            let other_index = index - 1;
                            editor.animation.swap(index, other_index);

                            environment
                                .update_var("Animation Index", (other_index + 1).to_string());
                        }
                    }
                }
                Demand::MoveAnimationDown => {
                    if let Some(index) = environment.get_typed_var::<usize>("Animation Index") {
                        let index = index - 1;
                        if subgame.is_some() && index < editor.animation.len() - 1 {
                            let other_index = index + 1;
                            editor.animation.swap(index, other_index);

                            environment
                                .update_var("Animation Index", (other_index + 1).to_string());
                        }
                    }
                }
                Demand::SetImageFile => {
                    menu_actions.push(menu::Action::SetImageFile);
                }
                Demand::SetMusicFile => {
                    menu_actions.push(menu::Action::SetMusicFile);
                }
                // Menu Actions
                Demand::Quit => {
                    menu_actions.push(menu::Action::Quit);
                }
                Demand::Play => {
                    menu_actions.push(menu::Action::Play);
                    music_maker.actions.insert(music::Action::PlayPhrase);
                }
                Demand::Pause => {
                    menu_actions.push(menu::Action::Pause);
                    music_maker.actions.insert(music::Action::PausePhrase);
                }
                Demand::Stop => {
                    menu_actions.push(menu::Action::Stop);
                    music_maker.actions.insert(music::Action::StopPhrase);
                }
                Demand::MoveToGame { name } => {
                    menu_actions.push(menu::Action::MoveToGame { name });
                }
                Demand::FadeToGame { name } => {
                    menu_actions.push(menu::Action::FadeToGame { name });
                }
                Demand::FadeOut => {
                    menu_actions.push(menu::Action::FadeOut);
                }
                Demand::BackInQueue => {
                    menu_actions.push(menu::Action::BackInQueue);
                }
                Demand::NextInQueue => {
                    menu_actions.push(menu::Action::NextInQueue);
                }
                Demand::AddToQueue { name } => {
                    menu_actions.push(menu::Action::AddToQueue { name });
                }
                Demand::ResetQueue => {
                    menu_actions.push(menu::Action::ResetQueue);
                }
                Demand::ClearArt => {
                    draw_tool.tracker.temp_clear = true;
                }
                Demand::SaveArt =>
                {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(subgame) = subgame {
                        unsafe {
                            use image::ImageEncoder;
                            let mut buff = Vec::new();
                            let enc = image::codecs::png::PngEncoder::new(&mut buff);
                            let image = &subgame.assets.image;

                            enc.write_image(
                                &image.bytes,
                                image.width as _,
                                image.height as _,
                                image::ColorType::Rgba8,
                            )
                            .unwrap();
                            let s = BaseEncoder.encode(&buff);
                            let data = crate::JsObject::string(&s);
                            crate::hi_from_wasm(data);
                            draw_tool.tracker.temp_save = true;
                        }
                    }
                }
                Demand::PlayPhrase => {
                    music_maker.actions.insert(music::Action::PlayPhrase);
                }
                Demand::PausePhrase => {
                    music_maker.actions.insert(music::Action::PausePhrase);
                }
                Demand::StopPhrase => {
                    music_maker.actions.insert(music::Action::StopPhrase);
                }
                Demand::PreviousInstrument => {
                    music_maker
                        .actions
                        .insert(music::Action::PreviousInstrument);
                }
                Demand::NextInstrument => {
                    music_maker.actions.insert(music::Action::NextInstrument);
                }
                Demand::PreviousTrack => {
                    music_maker.actions.insert(music::Action::PreviousTrack);
                }
                Demand::NextTrack => {
                    music_maker.actions.insert(music::Action::NextTrack);
                }
            }
        }
    }

    for i in 0..game.members.len() {
        // TODO: Handle motion directly after actions? 1 at a time or all at once?

        let move_to = |x: f32, other: f32, velocity: f32| {
            if (x - other).abs() > velocity.abs() {
                x + velocity
            } else {
                other
            }
        };

        game.members[i].motion = match game.members[i].motion.clone() {
            ActiveMotion::Go { direction, speed } => {
                let speed_multiplier = match speed {
                    Speed::VerySlow => 0.25,
                    Speed::Slow => 0.5,
                    Speed::Normal => 1.0,
                    Speed::Fast => 2.0,
                    Speed::VeryFast => 4.0,
                };
                let speed_constant = 2.0;
                let speed_value = speed_constant * speed_multiplier;
                let movement = match direction {
                    Direction::East => Vec2::new(speed_value, 0.0),
                    Direction::SouthEast => Vec2::new(speed_value, speed_value),
                    Direction::South => Vec2::new(0.0, speed_value),
                    Direction::SouthWest => Vec2::new(-speed_value, speed_value),
                    Direction::West => Vec2::new(-speed_value, 0.0),
                    Direction::NorthWest => Vec2::new(-speed_value, -speed_value),
                    Direction::North => Vec2::new(0.0, -speed_value),
                    Direction::NorthEast => Vec2::new(speed_value, -speed_value),
                };
                game.members[i].position += movement;
                ActiveMotion::Go { direction, speed }
            }
            ActiveMotion::GoToPoint { point, speed } => {
                let speed_multiplier = match speed {
                    Speed::VerySlow => 0.25,
                    Speed::Slow => 0.5,
                    Speed::Normal => 1.0,
                    Speed::Fast => 2.0,
                    Speed::VeryFast => 4.0,
                };
                let speed_constant = 2.0;
                let speed_value = speed_constant * speed_multiplier;
                let target_vector = Vec2::from(point) - game.members[i].position;
                let d = (target_vector.x.powf(2.0) + target_vector.y.powf(2.0)).sqrt();
                let velocity = Vec2 {
                    x: target_vector.x / d * speed_value,
                    y: target_vector.y / d * speed_value,
                };
                game.members[i].position.x =
                    move_to(game.members[i].position.x, point.x as f32, velocity.x);
                game.members[i].position.y =
                    move_to(game.members[i].position.y, point.y as f32, velocity.y);
                ActiveMotion::GoToPoint { point, speed }
            }
            ActiveMotion::Attach { name, offset } => {
                let maybe = game
                    .members
                    .iter()
                    .find(|member| member.name == *name)
                    .map(|other| other.position);
                if let Some(point) = maybe {
                    game.members[i].position = point + offset;
                }
                ActiveMotion::Attach { name, offset }
            }
            ActiveMotion::Target {
                name,
                offset,
                speed,
            } => {
                let maybe = game
                    .members
                    .iter()
                    .find(|member| member.name == *name)
                    .map(|other| other.position);
                if let Some(point) = maybe {
                    let speed_multiplier = match speed {
                        Speed::VerySlow => 0.25,
                        Speed::Slow => 0.5,
                        Speed::Normal => 1.0,
                        Speed::Fast => 2.0,
                        Speed::VeryFast => 4.0,
                    };
                    let speed_constant = 2.0;
                    let speed_value = speed_constant * speed_multiplier;
                    let target_vector = point - game.members[i].position;
                    let d = (target_vector.x.powf(2.0) + target_vector.y.powf(2.0)).sqrt();
                    let velocity = Vec2 {
                        x: target_vector.x / d * speed_value,
                        y: target_vector.y / d * speed_value,
                    };
                    game.members[i].position.x =
                        move_to(game.members[i].position.x, point.x, velocity.x);
                    game.members[i].position.y =
                        move_to(game.members[i].position.y, point.y, velocity.y);
                }
                ActiveMotion::Target {
                    name,
                    offset,
                    speed,
                }
            }
            ActiveMotion::Wiggle {
                area,
                speed,
                handling,
            } => {
                // TODO: Try not to overlap
                let speed_multiplier = match speed {
                    Speed::VerySlow => 0.25,
                    Speed::Slow => 0.5,
                    Speed::Normal => 1.0,
                    Speed::Fast => 2.0,
                    Speed::VeryFast => 4.0,
                };
                let speed_constant = 2.0;
                let speed_value = speed_constant * speed_multiplier;
                let x = macroquad::rand::gen_range(-speed_value, speed_value);
                let y = macroquad::rand::gen_range(-speed_value, speed_value);
                let pos = game.members[i].position;
                // TODO: Handle size of object, incl bg sizes
                let area2 = constrained_area(&game.assets.image, &game.members[i], area);
                if area2.w > 0.0 && area2.h > 0.0 && area2.contains(pos) {
                    game.members[i].position.x += x;
                    game.members[i].position.y += y;
                } else {
                    //game.members[i].x = move_to(game.members[i].x, area.centre().x, velocity.x);
                    //game.members[i].y = move_to(game.members[i].y, area.centre().y, velocity.y);

                    let centre = {
                        let c = area.centre();
                        Vec2::new(c[0], c[1])
                    };
                    let target_vector: Vec2 = centre - game.members[i].position;
                    let d = (target_vector.x.powf(2.0) + target_vector.y.powf(2.0)).sqrt();
                    let velocity = Vec2 {
                        x: target_vector.x / d * speed_value,
                        y: target_vector.y / d * speed_value,
                    };
                    game.members[i].position.x =
                        move_to(game.members[i].position.x, area.centre()[0], velocity.x);
                    game.members[i].position.y =
                        move_to(game.members[i].position.y, area.centre()[1], velocity.y);
                }
                ActiveMotion::Wiggle {
                    area,
                    speed,
                    handling,
                }
            }
            ActiveMotion::Insect {
                area,
                speed,
                handling,
                mut velocity,
            } => {
                // TODO: Try not to overlap
                let speed_multiplier = match speed {
                    Speed::VerySlow => 0.25,
                    Speed::Slow => 0.5,
                    Speed::Normal => 1.0,
                    Speed::Fast => 2.0,
                    Speed::VeryFast => 4.0,
                };
                let speed_constant = 2.0;
                let speed_value = speed_constant * speed_multiplier;
                if velocity == Vec2::default() || game.rng.number_in_range(0.0, 1.0) < 0.1 {
                    // TODO: generate velocity from possible directions
                    let x = macroquad::rand::gen_range(-speed_value, speed_value);
                    let y = macroquad::rand::gen_range(-speed_value, speed_value);
                    velocity = Vec2::new(x, y);
                }
                let pos = game.members[i].position;
                // TODO: Handle size of object, incl bg sizes
                let area2 = constrained_area(&game.assets.image, &game.members[i], area);
                if area2.w > 0.0 && area2.h > 0.0 && area2.contains(pos) {
                    game.members[i].position += velocity;
                } else {
                    //game.members[i].x = move_to(game.members[i].x, area.centre().x, velocity.x);
                    //game.members[i].y = move_to(game.members[i].y, area.centre().y, velocity.y);

                    let target_vector = Vec2 {
                        x: area.centre()[0] - game.members[i].position.x,
                        y: area.centre()[1] - game.members[i].position.y,
                    };
                    let d = (target_vector.x.powf(2.0) + target_vector.y.powf(2.0)).sqrt();
                    velocity = Vec2 {
                        x: target_vector.x / d * speed_value,
                        y: target_vector.y / d * speed_value,
                    };
                    game.members[i].position.x =
                        move_to(game.members[i].position.x, area.centre()[0], velocity.x);
                    game.members[i].position.y =
                        move_to(game.members[i].position.y, area.centre()[1], velocity.y);
                }
                ActiveMotion::Insect {
                    area,
                    speed,
                    handling,
                    velocity,
                }
            }
            ActiveMotion::Reflect {
                area,
                speed,
                handling,
                mut velocity,
            } => {
                // TODO: Try not to overlap
                let speed_multiplier = match speed {
                    Speed::VerySlow => 0.25,
                    Speed::Slow => 0.5,
                    Speed::Normal => 1.0,
                    Speed::Fast => 2.0,
                    Speed::VeryFast => 4.0,
                };
                let speed_constant = 2.0;
                let speed_value = speed_constant * speed_multiplier;
                let constrained_area = constrained_area(&game.assets.image, &game.members[i], area);
                let _pos = game.members[i].position;

                if velocity == Vec2::default() {
                    // TODO: generate velocity from possible directions
                    let x = macroquad::rand::gen_range(-speed_value, speed_value);
                    let y = macroquad::rand::gen_range(-speed_value, speed_value);
                    velocity = Vec2::new(x, y);
                    log::debug!("INITIAL: {:?}", velocity);
                    // TODO:
                    /*if !constrained_area.contains(pos) {
                        let target_vector = area.centre() - game.members[i].position;
                        let d = (target_vector.x.powf(2.0) + target_vector.y.powf(2.0)).sqrt();
                        velocity = Vec2 {
                            x: target_vector.x / d * speed_value,
                            y: target_vector.y / d * speed_value,
                        };
                    }*/
                }

                if constrained_area.w <= 0.0 {
                    velocity.x = 0.0;
                }
                if constrained_area.h <= 0.0 {
                    velocity.y = 0.0;
                }
                // TODO: Handle size of object, incl bg sizes
                game.members[i].position += velocity;

                if game.members[i].position.x > constrained_area.x + constrained_area.w {
                    velocity.x = -velocity.x.abs();
                }
                if game.members[i].position.x < constrained_area.x {
                    velocity.x = velocity.x.abs();
                }
                log::debug!(
                    "{:?}, {:?}, {}",
                    game.members[i].position.x,
                    constrained_area,
                    velocity.x
                );
                if game.members[i].position.y > constrained_area.y + constrained_area.h {
                    velocity.y = -velocity.y.abs();
                }
                if game.members[i].position.y < constrained_area.y {
                    velocity.y = velocity.y.abs();
                }
                ActiveMotion::Reflect {
                    area,
                    speed,
                    handling,
                    velocity,
                }
            }
            ActiveMotion::Bounce {
                area,
                speed,
                handling,
                mut velocity,
            } => {
                // TODO: Try not to overlap
                let speed_multiplier = match speed {
                    Speed::VerySlow => 0.25,
                    Speed::Slow => 0.5,
                    Speed::Normal => 1.0,
                    Speed::Fast => 2.0,
                    Speed::VeryFast => 4.0,
                };
                let speed_constant = 2.0;
                let speed_value = speed_constant * speed_multiplier;
                let constrained_area = constrained_area(&game.assets.image, &game.members[i], area);

                let acceleration = speed_value / 15.0;

                let deft =
                    -(2.0 * acceleration * (game.members[i].position.y - constrained_area.y))
                        .sqrt();

                if velocity == Vec2::default() {
                    // TODO: generate velocity from possible directions
                    //let x = macroquad::rand::gen_range(-speed_value, speed_value)
                    // TODO: randomness
                    let one_or_minus_one = (game.rng.number_in_range(0, 2) * 2 - 1) as f32;
                    let x = speed_value * one_or_minus_one;
                    // TODO: Initial bounce?
                    //let y = -speed_value;
                    let y = if game.members[i].position.y > constrained_area.y {
                        deft
                    } else {
                        0.0
                    };
                    velocity = Vec2::new(x, y);
                }

                if constrained_area.w <= 0.0 {
                    velocity.x = 0.0;
                }
                // TODO: Handle size of object, incl bg sizes
                game.members[i].position += velocity;

                velocity.y += acceleration;

                if game.members[i].position.x > constrained_area.x + constrained_area.w {
                    velocity.x = -velocity.x.abs();
                }
                if game.members[i].position.x < constrained_area.x {
                    velocity.x = velocity.x.abs();
                }
                if game.members[i].position.y > constrained_area.y + constrained_area.h {
                    velocity.y = deft;
                }
                ActiveMotion::Bounce {
                    area,
                    speed,
                    handling,
                    velocity,
                }
            }
            ActiveMotion::Stop => ActiveMotion::Stop,
        };
    }

    // TODO: Should do this after action or here?
    for member in game.members.iter_mut() {
        if let Some(new_sprite) = member.animation.update() {
            member.sprite = new_sprite;
        }

        member.switch.apply(member.applied_switch);
    }

    game.frame_number += 1;

    (events_to_apply, menu_actions)
}

pub fn position_in_world(position: pixels::Position, camera: Camera) -> pixels::Position {
    let quad_camera = camera.to_quad_camera();
    quad_camera.screen_to_world(position.into()).into()
}

fn is_position_in_text(
    position: pixels::Position,
    text_position: Vec2,
    font: &BitmapFont,
    text: &str,
) -> bool {
    let size = pixels::Size::new(font.text_width(text), font.char_height);
    let rect = pixels::Rect::from_centre(text_position.into(), size);
    rect.contains_point(position)
}

fn is_position_in_member_image(position: pixels::Position, member: &Member, image: &Image) -> bool {
    is_position_in_sprite_sheet_image(position, member.position.into(), member.sprite, image)
}

pub fn is_position_in_sprite_sheet_image(
    position: pixels::Position,
    image_position: pixels::Position,
    sprite: Sprite,
    image: &Image,
) -> bool {
    if sprite.size == SpriteSize::Empty {
        return false;
    }

    let source_rect = sheet_source_rect(sprite);

    is_adjusted_subsection_square_active(image, position, image_position, source_rect)
}

fn do_members_collide(a: &Member, b: &Member, image: &Image, font: &BitmapFont) -> bool {
    match a.sprite.size {
        SpriteSize::Empty => {
            let a_rect = pixels::Rect::xywh(
                a.position.x,
                a.position.y,
                font.text_width(&a.text.contents),
                font.char_height,
            );
            match b.sprite.size {
                SpriteSize::Empty => {
                    let b_rect = pixels::Rect::xywh(
                        b.position.x,
                        b.position.y,
                        font.text_width(&b.text.contents),
                        font.char_height,
                    );
                    a_rect.collides(b_rect)
                }
                _ => {
                    let b_source = sheet_source_rect(b.sprite);
                    let obj = CollisionObject {
                        position: b.position.into(),
                        section: b_source,
                        grid: image,
                    };
                    obj.collides_with_rect(a_rect)
                }
            }
        }
        _ => match b.sprite.size {
            SpriteSize::Empty => {
                let b_rect = pixels::Rect::xywh(
                    b.position.x,
                    b.position.y,
                    font.text_width(&b.text.contents),
                    font.char_height,
                );
                let a_source = sheet_source_rect(a.sprite);
                let obj = CollisionObject {
                    position: a.position.into(),
                    section: a_source,
                    grid: image,
                };
                obj.collides_with_rect(b_rect)
            }
            _ => {
                let a_source = sheet_source_rect(a.sprite);
                let a_obj = CollisionObject {
                    position: a.position.into(),
                    section: a_source,
                    grid: image,
                };
                let b_source = sheet_source_rect(b.sprite);
                let b_obj = CollisionObject {
                    position: b.position.into(),
                    section: b_source,
                    grid: image,
                };

                a_obj.collides_with_other(b_obj)
            }
        },
    }
}

pub fn is_position_in_sized_area(
    try_position: pixels::Position,
    target_position: Vec2,
    size: pixels::Size,
) -> bool {
    let rect = pixels::Rect::from_centre(target_position.into(), size);

    rect.contains_point(try_position)
}

fn constrained_area(image: &Image, member: &Member, area: pixels::Rect) -> Rect {
    let pixel_size = sprite_size_in_pixels(member.sprite.size);
    let width = pixel_size.w as f32;
    let height = pixel_size.h as f32;

    let area: Rect = area.into();

    let source_rect = sheet_source_rect(member.sprite);

    // TODO: ?
    let top_left_of_source = source_rect.min;
    let mut toppest = height;
    let mut bottomest: f32 = 0.0;
    let mut leftest = width;
    let mut rightest: f32 = 0.0;
    for x in 0..pixel_size.w {
        for y in 0..pixel_size.h {
            let pex = get_pixel(
                image,
                x + top_left_of_source.x as u32,
                y + top_left_of_source.y as u32,
            );
            if pex.a != 0.0 {
                toppest = toppest.min(y as f32);
                bottomest = bottomest.max(y as f32);
                leftest = leftest.min(x as f32);
                rightest = rightest.max(x as f32);
            }
        }
    }

    let mut x = area.x + width / 2.0 - leftest;
    let mut w = area.x + area.w + width / 2.0 - rightest - x;
    if w < 0.0 {
        let expected_centre = width / 2.0;
        let actual_centre = (leftest + rightest + 1.0) / 2.0;
        let diff = expected_centre - actual_centre;
        x = area.x + area.w / 2.0 + diff; // + (bottomest - toppest) / 2.0;
        w = 0.0;
    }

    let mut y = area.y + height / 2.0 - toppest;
    let mut h = area.y + area.h + height / 2.0 - bottomest - y;
    // TODO: This doesn't centre based on pixels in image
    if h < 0.0 {
        let expected_centre = height / 2.0;
        let actual_centre = (bottomest + toppest + 1.0) / 2.0;
        let diff = expected_centre - actual_centre;
        y = area.y + area.h / 2.0 + diff; // + (bottomest - toppest) / 2.0;
        h = 0.0;
    }

    Rect { x, y, w, h }
}

// TODO: Double check everywhere this is called isn't hiding issues
fn get_pixel(image: &Image, x: u32, y: u32) -> Colour {
    if x < image.width as u32 && y < image.height as u32 {
        image.get_pixel(x, y)
    } else {
        colours::BLANK
    }
}

pub fn rename_in_todo_list(todo_list: &mut Vec<Chore>, from: &str, to: &str) {
    for chore in todo_list {
        for question in &mut chore.questions {
            match question {
                Question::IsSwitchSetTo { name, .. }
                | Question::IsCollidingWith(CollisionWith::Member { name }) => {
                    if name == from {
                        *name = to.to_owned();
                    }
                }
                _ => {}
            }
        }
        for demand in &mut chore.demands {
            match demand {
                Demand::Motion(Motion::JumpTo(JumpLocation::Member { name }))
                | Demand::Motion(Motion::Target { name, .. })
                | Demand::Motion(Motion::AttachFromPositions { name, .. }) => {
                    if name == from {
                        *name = to.to_owned();
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ChoreId {
    pub member: usize,
    pub chore: usize,
}

impl ChoreId {
    fn new(member: usize, chore: usize) -> ChoreId {
        ChoreId { member, chore }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct QuestionId {
    pub member: usize,
    pub chore: usize,
    pub question: usize,
}

impl QuestionId {
    fn new(member: usize, chore: usize, question: usize) -> QuestionId {
        QuestionId {
            member,
            chore,
            question,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DemandId {
    pub member: usize,
    pub chore: usize,
    pub demand: usize,
}

impl DemandId {
    fn new(member: usize, chore: usize, demand: usize) -> DemandId {
        DemandId {
            member,
            chore,
            demand,
        }
    }
}

fn serialised_members(members: Vec<Member>) -> Vec<serial::Member> {
    let mut serialised_members = Vec::new();
    for member in members {
        serialised_members.push(serial::Member {
            name: member.name.to_owned(),
            position: member.position.into(),
            sprite: member.sprite,
            text: serial::Text {
                contents: member.text.contents.to_owned(),
                colour: serial::Colour {
                    r: member.text.colour.r,
                    g: member.text.colour.g,
                    b: member.text.colour.b,
                    a: member.text.colour.a,
                },
            },
            todo_list: {
                let mut todo_list = Vec::new();
                for chore in &member.todo_list {
                    todo_list.push(serial::Chore {
                        questions: chore.questions.to_vec(),
                        demands: chore.demands.to_vec(),
                    });
                }

                for chore in todo_list.iter_mut().take(serial::CHORE_COUNT) {
                    for question_index in (0..serial::QUESTION_COUNT).rev() {
                        if chore.questions[question_index] == Question::None {
                            chore.questions.remove(question_index);
                        } else {
                            break;
                        }
                    }
                    for demand_index in (0..serial::DEMAND_COUNT).rev() {
                        if chore.demands[demand_index] == Demand::None {
                            chore.demands.remove(demand_index);
                        } else {
                            break;
                        }
                    }
                }

                for chore_index in (0..serial::CHORE_COUNT).rev() {
                    if todo_list[chore_index].questions.is_empty()
                        && todo_list[chore_index].demands.is_empty()
                    {
                        todo_list.remove(chore_index);
                    } else {
                        break;
                    }
                }
                todo_list
            },
        });
    }
    serialised_members
}

fn playable_members(members: Vec<serial::Member>) -> Vec<Member> {
    let mut playable_members = Vec::new();
    for member in members {
        playable_members.push(Member {
            name: member.name.to_owned(),
            position: member.position.into(),
            sprite: member.sprite,
            text: Text {
                contents: member.text.contents.to_owned(),
                colour: Colour {
                    r: member.text.colour.r,
                    g: member.text.colour.g,
                    b: member.text.colour.b,
                    a: member.text.colour.a,
                },
            },
            todo_list: {
                let mut todo_list = default_todo_list();
                for (chore_index, chore) in
                    todo_list.iter_mut().enumerate().take(serial::CHORE_COUNT)
                {
                    for question_index in 0..serial::QUESTION_COUNT {
                        chore.questions[question_index] = member
                            .todo_list
                            .get(chore_index)
                            .and_then(|chore| chore.questions.get(question_index))
                            .cloned()
                            .unwrap_or(Question::None);
                    }
                    for demand_index in 0..serial::DEMAND_COUNT {
                        chore.demands[demand_index] = member
                            .todo_list
                            .get(chore_index)
                            .and_then(|chore| chore.demands.get(demand_index))
                            .cloned()
                            .unwrap_or(Demand::None);
                    }
                }
                todo_list
            },
            ..Default::default()
        });
    }
    playable_members
}

pub fn cartridge_from_game(game: Game) -> Cartridge {
    // TODO: Looped
    let music = game.assets.music_string.map(|music_data| Music {
        data: music_data,
        looped: false,
    });
    Cartridge {
        format_version: 0,
        members: serialised_members(game.members),
        published: true,
        length: game.length,
        size: game.size,
        intro_text: game.intro_text,
        font: game.assets.font_string,
        image: game.assets.image_string,
        music,
        sounds: HashMap::default(),
        asset_filenames: game.assets.filenames,
    }
}

pub fn game_from_cartridge(cartridge: Cartridge, rng: SeededRng) -> Game {
    // TODO: Don't lose looped info
    let music = cartridge.music.map(|music| music.data);
    Game {
        members: playable_members(cartridge.members),
        assets: Assets::from_strings(
            cartridge.image,
            cartridge.font,
            music,
            cartridge.asset_filenames,
        ),
        size: cartridge.size,
        length: cartridge.length,
        win_status: WinStatus::NotYetWon,
        triggered_questions: HashSet::new(),
        frame_number: 0,
        intro_text: cartridge.intro_text,
        rng,
    }
}
