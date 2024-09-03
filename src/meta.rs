use crate::art::SpriteSize;
use crate::{colours, BootInfo};

use crate::doodle::{draw_using_brush, DrawMode, DrawTool, Fill};
use crate::drawer::{page_width_for_sprite, sprite_size_in_pixels};
use crate::edit;
use crate::edit::{get_typed_variable, hovered_in_general_area, sprite_from_context, Editor};
use crate::err::WhyResult;
use crate::history;
use crate::history::{Event, Step, StepDirection};
use crate::inp::{Input, Mouse, BACKSPACE_CODE, CTRL_Y_CHAR, CTRL_Z_CHAR, FIRST_LEGIT_KEY};
use crate::menu;
use crate::music::{MusicMaker, POTENTIAL_NOTE_OFFSET};
use crate::nav::{Link, Navigation};
use crate::pixels;
use crate::play::{
    self, is_position_in_sized_area, rename_in_todo_list, update_game, Assets, DifficultyLevel,
    SoundQueue,
};
use crate::seeded_rng::SeededRng;
use crate::serial::{self, ImageString, SoundString};
use crate::time::TimeKeeping;
use crate::AudioPlayer;

use crate::{game_from_cartridge, temp_load, temp_save, texture_from_bytes};
use base64::{engine::general_purpose::STANDARD_NO_PAD as BaseEncoder, Engine};
use macroquad::{input::KeyCode, logging as log, math::Vec2};
use rodio::{Decoder, Sink};

use std::io;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

pub const OUTER_WIDTH: u32 = 384;
pub const OUTER_HEIGHT: u32 = 216;
pub const OUTER_SIZE: pixels::Size = pixels::Size::new(OUTER_WIDTH, OUTER_HEIGHT);
pub const OUTER_CENTRE: pixels::Position = OUTER_SIZE.centre();
pub const INNER_WIDTH: u32 = 256;
pub const INNER_HEIGHT: u32 = 144;
pub const INNER_SIZE: pixels::Size = pixels::Size::new(INNER_WIDTH, INNER_HEIGHT);
pub const INNER_CENTRE: pixels::Position = INNER_SIZE.centre();
pub const INITIAL_SCALE: f32 = 4.0;

pub const EDITABLE_SCREEN_NAME: &str = "{Screen}";
pub const SCREEN_NAME: &str = "{Screen}";
pub const CHOOSE_AREA_NAME: &str = "{Choose Area}";
pub const CHOOSE_POINT_NAME: &str = "{Choose Point}";
pub const PLAY_SCREEN_NAME: &str = "{Play Screen}";
pub const MUSIC_MAKER_NAME: &str = "{Music Maker}";

pub const DEFAULT_IMAGE_FILENAME: &str = "green.png";
pub const DEFAULT_FONT_FILENAME: &str = "pixolletta.png";
pub const INTRO_FONT_FILENAME: &str = "littleguy.png";

pub const FADE_LEN: i32 = 60;

pub enum MenuOutcome {
    Quit,
    None,
}

#[derive(Debug)]
pub enum Transition {
    FadeOut { game: play::Game, fade_left: i32 },
    FadeIn { game: play::Game, fade_left: i32 },
    None,
}

impl Transition {
    pub fn is_none(&self) -> bool {
        matches!(self, Transition::None)
    }
}

#[derive(Debug)]
pub struct Environment {
    pub score: i32,
    pub difficulty_level: DifficultyLevel,
    pub playback_rate: f64,
    pub context: HashMap<String, String>,
    pub rng: SeededRng,
}

impl Environment {
    /*pub fn new(rng: SeededRng) -> Metagame {
        // TODO: Repeated?
        let filtered_texture_from_image = |image: &Image| {
            let texture = Texture2D::from_image(image);
            texture.set_filter(FilterMode::Nearest);
            texture
        };
        let image = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, colours::WHITE);

        Metagame {
            queue: GameQueue::default(),
            next_game: None,
            transition: Transition::None,
            score: 0,
            difficulty_level: DifficultyLevel::default(),
            initial_game_name: "".to_string(),
            context: HashMap::new(),
            rng,
            draw_tool: DrawTool {
                paint_choices: Vec::new(),
                erase_paint: EditableImage {
                    texture: filtered_texture_from_image(&image),
                    image,
                },
                paint_index: 0,
                fill: None,
                temp_clear: false,
                temp_save: false,
            },
            music_maker: MusicMaker {
                actions: HashSet::new(),
                instruments: Vec::new(),
                page: 0,
                is_extended_keyboard: false,
                is_alternative_signature: false,
                note_length: 1,
                tempo: 120,
                track_index: 0,
                tracks: [
                    MakerTrack::default(),
                    MakerTrack::default(),
                    MakerTrack::default(),
                    MakerTrack::default(),
                ],
                should_refresh_song: false,
                is_solo_playback: true,
            },
        }
    }*/

    pub fn update_var<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.context.insert(key.into(), value.into());
    }

    pub fn update_var_as_debug<K: Into<String>, V: std::fmt::Debug>(&mut self, key: K, value: V) {
        self.context.insert(key.into(), format!("{:?}", value));
    }

    pub fn get_typed_var<T: FromStr>(&self, key: &str) -> Option<T> {
        get_typed_variable::<T>(&self.context, key)
    }

    pub fn _get_var_or_default<T: FromStr + std::default::Default>(&self, key: &str) -> T {
        get_typed_variable::<T>(&self.context, key).unwrap_or_default()
    }

    pub fn get_var_for_text(&self, key: &str) -> Option<String> {
        self.context.get(key).map(|s| s.to_owned())
    }

    pub fn has_valid_var<T: FromStr>(&self, key: &str) -> bool {
        get_typed_variable::<T>(&self.context, key).is_some()
    }

    pub fn init_vars(&mut self, subgame: &play::Game, boot_info: &BootInfo) {
        self.update_var("Game File Name", boot_info.initial_subgame.game.clone());

        // TODO: Or initial_game?
        self.update_var("Collection", boot_info.initial_subgame.collection.clone());
        self.update_var_as_debug("Game Size", subgame.size);
        self.update_var_as_debug("Length", subgame.length);

        self.update_var_as_debug("Difficulty", self.difficulty_level);

        self.update_var("Image", DEFAULT_IMAGE_FILENAME);
        self.update_var("Font", DEFAULT_FONT_FILENAME);

        self.update_var("Game", boot_info.initial_subgame.game.clone());
    }
}

pub async fn update_metagame(
    environment: &mut Environment,
    navigation: &mut Navigation,
    editors: (&mut Editor, &mut Editor),
    draw_tool: &mut DrawTool,
    music_maker: &mut MusicMaker,
    game: &mut play::Game,
    subgame: &mut play::Game,
    input: &Input,
    audio_player: &mut AudioPlayer,
    transition: &mut Transition,
    time_keeping: TimeKeeping,
) -> WhyResult<MenuOutcome> {
    let mut events_to_apply = Vec::new();
    let mut sounds_to_play = SoundQueue::Ready {
        sounds: HashSet::new(),
    };
    let (editor, dummy_editor) = editors;
    editor.edit_text_index = None;

    match transition {
        Transition::FadeIn {
            game: older_game, ..
        }
        | Transition::FadeOut {
            game: older_game, ..
        } => {
            update_game(
                older_game,
                input.outer,
                &mut sounds_to_play,
                editor,
                environment,
                navigation,
                draw_tool,
                music_maker,
                Some(subgame),
            );
        }
        Transition::None => {}
    }

    let (mut new_events, menu_actions) = update_game(
        game,
        input.outer,
        &mut sounds_to_play,
        editor,
        environment,
        navigation,
        draw_tool,
        music_maker,
        Some(subgame),
    );

    // TODO: Rework using an question/demand and undoable event
    if environment.context["Game Size"] == "Small" {
        subgame.size = play::Size::Small;
    }
    if environment.context["Game Size"] == "Big" {
        subgame.size = play::Size::Big;
    }
    if environment.context["Length"] == "Short" {
        subgame.length = play::Length::Short;
    }
    if environment.context["Length"] == "Long" {
        subgame.length = play::Length::Long;
    }
    if environment.context["Length"] == "Infinite" {
        subgame.length = play::Length::Infinite;
    }
    if environment.context["Difficulty"] == "Normal" {
        environment.difficulty_level = DifficultyLevel::Normal;
    }
    if environment.context["Difficulty"] == "Challenge" {
        environment.difficulty_level = DifficultyLevel::Challenge;
    }
    if environment.context["Difficulty"] == "Tough" {
        environment.difficulty_level = DifficultyLevel::Tough;
    }
    environment.playback_rate = environment.get_typed_var("Playback Rate").unwrap_or(1.0);

    events_to_apply.append(&mut new_events);

    if editor.inner_copy.is_some() {
        update_game(
            subgame,
            input.inner,
            &mut sounds_to_play,
            dummy_editor,
            environment,
            navigation,
            draw_tool,
            music_maker,
            None,
        );
    }

    // TODO: Sort this out
    if let Some(sink_player) = &mut audio_player.sink_player {
        match sounds_to_play {
            SoundQueue::Ready { sounds } => {
                // TODO: !
                for sound in sounds {
                    #[cfg(not(target_arch = "wasm32"))]
                    let data = macroquad::file::load_file(&format!("sounds/{}.ogg", sound))
                        .await
                        .unwrap();
                    #[cfg(target_arch = "wasm32")]
                    let data = sink_player.temp_loaded_sounds[&sound].clone();

                    let cursor = io::Cursor::new(data);
                    let source = Decoder::new(cursor).unwrap();
                    match Sink::try_new(&sink_player.stream_handle) {
                        Ok(sink) => {
                            sink.append(source);
                            sink_player.sfx_sinks.push(sink);
                        }
                        Err(error) => {
                            log::error!("SFX NOT WORKING: {:?}", error);
                        }
                    }
                }
            }
            SoundQueue::Stopped => {
                for s in &sink_player.sfx_sinks {
                    s.stop();
                }
            }
        }
    }

    // TODO: Mismatched if/else?
    // TODO: Not an editor thing?
    if let Some(i) = editor.edit_text_index {
        if i < game.members.len() {
            let text = &mut game.members[i].text.contents;

            // TODO: Wasm config
            #[cfg(target_arch = "wasm32")]
            if macroquad::input::is_key_down(KeyCode::Backspace) {
                text.pop();
            }

            for &ch in &input.chars_pressed {
                log::debug!("CH: {}", ch);
                // TODO: Wasm config
                if ch as u32 == BACKSPACE_CODE {
                    text.pop();
                } else if (ch as u32) >= FIRST_LEGIT_KEY {
                    text.push(ch);
                }
            }
        }
    } else if has_editable_screen(&game.members) && editor.inner_copy.is_none() {
        for &ch in &input.chars_pressed {
            if ch as u32 == BACKSPACE_CODE {
                events_to_apply.push(Event::RemoveCharacter {
                    index: editor.selected_index,
                });
            } else if (ch as u32) >= FIRST_LEGIT_KEY {
                events_to_apply.push(Event::AddCharacter {
                    index: editor.selected_index,
                    ch,
                });
            }
        }
    }

    if has_editable_screen(&game.members) && editor.inner_copy.is_none() {
        let is_in_select_mode = environment.context["Editor Mode"] == "Select";
        if is_in_select_mode && input.outer.left_button.is_pressed() {
            let hovered_indices = hovered_in_general_area(
                &subgame.members,
                input.inner.position,
                &subgame.assets.font,
            );

            if !hovered_indices.is_empty() {
                editor.index_tracker += 1;
                editor.index_tracker %= hovered_indices.len();
                if hovered_indices != editor.previous_hovered_indices {
                    editor.index_tracker = 0;
                }
                if editor.selected_index == hovered_indices[editor.index_tracker] {
                    editor.index_tracker += 1;
                    editor.index_tracker %= hovered_indices.len();
                }
                editor.selected_index = hovered_indices[editor.index_tracker];
                editor.previous_hovered_indices = hovered_indices;
            }
        }

        if input.outer.middle_button.is_pressed() {
            environment
                .context
                .insert("Editor Mode".to_string(), "Move".to_string());
        } else if input.outer.middle_button.is_released() {
            environment
                .context
                .insert("Editor Mode".to_string(), "Select".to_string());
        }

        // TODO: Will this work for middle mouse clicks?
        let is_in_move_mode = environment.context["Editor Mode"] == "Move";

        if (is_in_move_mode && input.outer.left_button.is_released())
            || input.outer.middle_button.is_released()
        {
            events_to_apply.push(Event::MoveMember {
                index: editor.selected_index,
                from: editor.original_position,
                to: subgame.members[editor.selected_index].position,
            })
        }

        if is_in_move_mode
            && (input.outer.left_button.is_pressed() || input.outer.middle_button.is_pressed())
        {
            editor.original_position = subgame.members[editor.selected_index].position;
            editor.original_mouse_position = input.inner.position;
        }
        if is_in_move_mode
            && (input.outer.left_button.is_down() || input.outer.middle_button.is_down())
        {
            let movement: Vec2 = input.inner.drag.into();
            // TODO: Use current mouse position vs original mouse position for diff
            let diff = input.inner.position - editor.original_mouse_position;
            if macroquad::input::is_key_down(KeyCode::LeftControl) {
                if diff.x.abs() > diff.y.abs() {
                    subgame.members[editor.selected_index].position.x += movement.x;
                    subgame.members[editor.selected_index].position.y = editor.original_position.y;
                } else if diff.y.abs() > diff.x.abs() {
                    subgame.members[editor.selected_index].position.x = editor.original_position.x;
                    subgame.members[editor.selected_index].position.y += movement.y;
                } else {
                    subgame.members[editor.selected_index].position += movement;
                }
            } else {
                subgame.members[editor.selected_index].position += movement;
            };
        }
    }

    if game.music_maker_member().is_some() {
        music_maker.update_keyboard(environment, &mut events_to_apply);
        music_maker.update_signature(environment, &mut events_to_apply);

        music_maker.editing_position.page =
            get_typed_variable::<u32>(&environment.context, "Half Page").unwrap() - 1;

        let is_in_music_maker_area = MusicMaker::is_mouse_hovering(game, input.outer.position);

        let note_height = music_maker.note_height();
        let note_count = music_maker.note_count();
        let note_adjust = music_maker.note_adjust();

        let y = ((216 - input.outer.position.y) - 40) / note_height;
        let note = y.max(0).min(note_count - 1) as u8 + note_adjust;

        music_maker.tracker.has_valid_intended_note = false;
        music_maker.intended_note.pitch = note;

        if input.rmb_held_down_for > 8 {
            environment.update_var("Music Mode", "Remove");
        }
        if input.outer.right_button.is_released() {
            environment.update_var("Music Mode", "Add");
        }

        music_maker.try_to_place_note(
            environment,
            input,
            audio_player,
            time_keeping,
            &mut events_to_apply,
        );

        if environment.context["Music Mode"] == "Add" && input.outer.left_button.is_released() {
            music_maker.tracker.has_preview_note = false;
        }

        if music_maker.should_note_be_stopped(time_keeping) {
            music_maker.stop_last_maker_key(audio_player);
        }

        if is_in_music_maker_area
            && environment.context["Music Mode"] == "Add"
            && input.outer.left_button.is_down()
        {
            audio_player.set_record_volume(0.0);
        }

        if is_in_music_maker_area
            && (environment.context["Music Mode"] == "Remove" && input.outer.left_button.is_down())
            || input.outer.right_button.is_down()
        {
            audio_player.stop_all_record_notes();
        }

        if let Some(tempo) = get_typed_variable::<u32>(&environment.context, "Tempo") {
            music_maker.update_tempo(environment, tempo);
        }

        music_maker.update_playback(environment);

        if is_in_music_maker_area
            && (input.outer.left_button.is_released()
                || input.outer.middle_button.is_released()
                || input.outer.right_button.is_released())
        {
            music_maker.refresh_song();
        }
    }

    // Extracted from render code
    for member in &game.members {
        if member.text.contents == "{Sprite Sheet}" {
            update_using_sprite_sheet(member.position, input.outer, &mut environment.context);
        }

        if member.text.contents == "{Edit Sprite}" {
            draw_tool.draw_stuff(
                environment,
                input,
                member.position,
                &mut subgame.assets,
                &mut events_to_apply,
                subgame.size,
            );
        }

        if member.text.contents == CHOOSE_AREA_NAME {
            if input.outer.middle_button.is_pressed() {
                environment.update_var("MinX", input.inner.position.x.to_string());
                environment.update_var("MinY", input.inner.position.y.to_string());
            }
            if input.outer.middle_button.is_down() {
                environment.update_var("MaxX", input.inner.position.x.to_string());
                environment.update_var("MaxY", input.inner.position.y.to_string());
            }
        }
        if member.text.contents == CHOOSE_POINT_NAME && input.outer.middle_button.is_down() {
            environment.update_var("X", input.inner.position.x.to_string());
            environment.update_var("Y", input.inner.position.y.to_string());
        }
    }

    #[derive(Debug, PartialEq)]
    enum HistoryAction {
        Undo,
        Redo,
        None,
    }

    let history_action = {
        //log::debug!("CHARS PRESSED: {:?}", chars_pressed);
        let is_ctrl_z_pressed = input.chars_pressed.contains(&CTRL_Z_CHAR);
        #[cfg(target_arch = "wasm32")]
        let is_ctrl_z_pressed = macroquad::input::is_key_down(KeyCode::LeftControl)
            && input.keyboard[&KeyCode::Z].is_repeated;
        let is_ctrl_y_pressed = input.chars_pressed.contains(&CTRL_Y_CHAR);
        #[cfg(target_arch = "wasm32")]
        let is_ctrl_y_pressed = macroquad::input::is_key_down(KeyCode::LeftControl)
            && input.keyboard[&KeyCode::Y].is_repeated;
        let is_shift_pressed = macroquad::input::is_key_down(KeyCode::LeftShift)
            || macroquad::input::is_key_down(KeyCode::RightShift);

        if is_ctrl_y_pressed
        /*|| (is_ctrl_z_pressed && is_shift_pressed)*/
        {
            log::debug!("REDO");
            HistoryAction::Redo
        } else if is_ctrl_z_pressed {
            log::debug!("UNDO: {}", editor.undo_stack.len());
            HistoryAction::Undo
        } else {
            HistoryAction::None
        }
    };

    if history_action == HistoryAction::Undo {
        if let Some(step) = editor.undo_stack.pop() {
            let event = match step.direction {
                StepDirection::Forward => &step.back,
                StepDirection::Back => &step.forward,
            };

            apply_event(
                event,
                &mut subgame.members,
                &mut editor.selected_index,
                &mut environment.context,
                music_maker,
                &mut subgame.assets,
            );

            let name = match step.direction {
                StepDirection::Forward => step.forward.to_string(),
                StepDirection::Back => format!("Undo {}", step.forward),
            };
            log::debug!("Event: Undo '{}'", name);

            editor.redo_stack.push(step);
        }
    } else if history_action == HistoryAction::Redo {
        music_maker.refresh_song();
        if let Some(step) = editor.redo_stack.pop() {
            let event = match step.direction {
                StepDirection::Forward => &step.forward,
                StepDirection::Back => &step.back,
            };

            apply_event(
                event,
                &mut subgame.members,
                &mut editor.selected_index,
                &mut environment.context,
                music_maker,
                &mut subgame.assets,
            );

            let name = match step.direction {
                StepDirection::Forward => step.forward.to_string(),
                StepDirection::Back => format!("Undo {}", step.forward),
            };
            log::debug!("Event: Redo '{}'", name);

            editor.undo_stack.push(step);
        }
    }

    if history_action != HistoryAction::None || !events_to_apply.is_empty() {
        music_maker.refresh_song();
    }

    handle_events(
        events_to_apply,
        subgame,
        &mut editor.selected_index,
        &mut environment.context,
        &mut editor.undo_stack,
        &mut editor.redo_stack,
        music_maker,
    );

    apply_menu_actions(
        menu_actions,
        editor,
        environment,
        navigation,
        game,
        subgame,
        audio_player,
        transition,
    )
    .await
}

pub fn has_editable_screen(members: &[play::Member]) -> bool {
    // TODO: ?
    members
        .iter()
        .any(|m| m.text.contents == EDITABLE_SCREEN_NAME)
}

pub fn is_screen_member_name(name: &str) -> bool {
    name == SCREEN_NAME
        || name == EDITABLE_SCREEN_NAME
        || name == CHOOSE_AREA_NAME
        || name == CHOOSE_POINT_NAME
        || name == PLAY_SCREEN_NAME
}

pub fn apply_event(
    event: &history::Event,
    members: &mut Vec<edit::Member>,
    selected_index: &mut usize,
    context_variables: &mut HashMap<String, String>,
    music_maker: &mut MusicMaker,
    assets: &mut Assets,
) -> bool {
    match event {
        Event::AddMember { index, member } => {
            if let Some(index) = index {
                members.insert(*index, member.clone());
                *selected_index = *index;
            } else {
                members.push(member.clone());
                *selected_index = members.len() - 1;
            }
            true
        }
        Event::RemoveMember { index } => {
            if members.len() > 1 && members.len() > *index {
                members.remove(*index);
                if *selected_index >= members.len() {
                    *selected_index -= 1;
                }
                true
            } else {
                false
            }
        }
        Event::MoveMember {
            index,
            from: _from,
            to,
        } => {
            members[*index].position = *to;
            true
        }
        Event::RenameMember { index, from, to } => {
            // TODO: Block loading games which have 2 members with the same name
            members[*index].name = to.to_owned();
            for member in members {
                rename_in_todo_list(&mut member.todo_list, from, to);
            }
            true
        }
        Event::UpdateChore { id, chore } => {
            members[id.member].todo_list[id.chore] = *chore.clone();
            true
        }
        Event::MoveChoreUp { id } => {
            if id.chore == 0 {
                false
            } else {
                members[id.member].todo_list.swap(id.chore, id.chore - 1);
                context_variables.insert("Chore Index".to_owned(), (id.chore).to_string());
                true
            }
        }
        Event::MoveChoreDown { id } => {
            if id.chore == serial::CHORE_COUNT - 1 {
                false
            } else {
                members[id.member].todo_list.swap(id.chore, id.chore + 1);
                context_variables.insert("Chore Index".to_owned(), (id.chore + 2).to_string());
                true
            }
        }
        Event::MoveQuestionUp { id } => {
            if id.question == 0 {
                false
            } else {
                members[id.member].todo_list[id.chore]
                    .questions
                    .swap(id.question, id.question - 1);
                context_variables.insert("Question Index".to_owned(), (id.question).to_string());
                true
            }
        }
        Event::MoveQuestionDown { id } => {
            if id.question == serial::QUESTION_COUNT - 1 {
                false
            } else {
                members[id.member].todo_list[id.chore]
                    .questions
                    .swap(id.question, id.question + 1);
                context_variables
                    .insert("Question Index".to_owned(), (id.question + 2).to_string());
                true
            }
        }
        Event::MoveDemandUp { id } => {
            if id.demand == 0 {
                false
            } else {
                members[id.member].todo_list[id.chore]
                    .demands
                    .swap(id.demand, id.demand - 1);
                context_variables.insert("Demand Index".to_owned(), (id.demand).to_string());
                true
            }
        }
        Event::MoveDemandDown { id } => {
            if id.demand == serial::DEMAND_COUNT - 1 {
                false
            } else {
                members[id.member].todo_list[id.chore]
                    .demands
                    .swap(id.demand, id.demand + 1);
                context_variables.insert("Demand Index".to_owned(), (id.demand + 2).to_string());
                true
            }
        }
        Event::UpdateQuestion { id, question } => {
            members[id.member].todo_list[id.chore].questions[id.question] = question.clone();
            true
        }
        Event::UpdateDemand { id, demand } => {
            members[id.member].todo_list[id.chore].demands[id.demand] = demand.clone();
            true
        }
        Event::AddCharacter { index, ch } => {
            members[*index].text.contents.push(*ch);
            true
        }
        Event::RemoveCharacter { index } => members[*index].text.contents.pop().is_some(),
        Event::SetStartSprite {
            index,
            from: _from,
            to,
        } => {
            members[*index].sprite = *to;
            true
        }
        Event::AddNote {
            editing_position,
            note,
        } => {
            music_maker.editing_position = *editing_position;
            context_variables.insert(
                "Half Page".to_string(),
                (music_maker.editing_position.page + 1).to_string(),
            );
            *music_maker.notes_mut() = music_maker
                .notes()
                .iter()
                .filter(|n| n.offset != note.offset)
                // TODO:s
                .copied()
                .collect();
            music_maker.notes_mut().push(*note);
            true
        }
        Event::RemoveNote {
            editing_position,
            note,
        } => {
            music_maker.editing_position = *editing_position;
            context_variables.insert(
                "Half Page".to_string(),
                (music_maker.editing_position.page + 1).to_string(),
            );
            *music_maker.notes_mut() = music_maker
                .notes()
                .iter()
                .filter(|n| n.offset != note.offset)
                .copied()
                .collect();
            true
        }
        Event::SwitchToExtendedKeyboard {
            editing_position,
            old_notes,
        } => {
            music_maker.editing_position = *editing_position;
            context_variables.insert(
                "Half Page".to_string(),
                (music_maker.editing_position.page + 1).to_string(),
            );
            context_variables.insert("Keyboard".to_owned(), "Extended".to_owned());
            music_maker.switch_to_extended_keyboard();
            *music_maker.notes_mut() = old_notes.clone();
            true
        }
        Event::SwitchToStandardKeyboard {
            editing_position, ..
        } => {
            music_maker.editing_position = *editing_position;
            context_variables.insert(
                "Half Page".to_string(),
                (music_maker.editing_position.page + 1).to_string(),
            );
            context_variables.insert("Keyboard".to_owned(), "Standard".to_owned());
            music_maker.switch_to_standard_keyboard();
            *music_maker.notes_mut() = music_maker
                .notes()
                .clone()
                .into_iter()
                .filter(|note| {
                    note.pitch >= POTENTIAL_NOTE_OFFSET && note.pitch < POTENTIAL_NOTE_OFFSET + 25
                })
                .collect();
            true
        }
        Event::SwitchToAlternativeSignature {
            editing_position,
            old_notes,
        } => {
            music_maker.editing_position = *editing_position;
            context_variables.insert(
                "Half Page".to_string(),
                (music_maker.editing_position.page + 1).to_string(),
            );
            context_variables.insert("Signature".to_owned(), "3/4".to_owned());
            music_maker.switch_to_alternative_signature();
            *music_maker.notes_mut() = old_notes.clone();
            *music_maker.notes_mut() = music_maker
                .notes()
                .clone()
                .into_iter()
                .filter(|note| note.offset < 24)
                .collect();
            true
        }
        Event::SwitchToStandardSignature {
            editing_position, ..
        } => {
            music_maker.editing_position = *editing_position;
            context_variables.insert(
                "Half Page".to_string(),
                (music_maker.editing_position.page + 1).to_string(),
            );
            context_variables.insert("Signature".to_owned(), "4/4".to_owned());
            music_maker.switch_to_standard_signature();
            true
        }
        Event::SetPixels {
            updates,
            left_to_right,
        } => {
            for (position, (from, to)) in updates.as_ref() {
                let to = if *left_to_right { to } else { from };
                assets
                    .image
                    .set_pixel(position.x as u32, position.y as u32, *to);
            }
            assets.texture.update(&assets.image);
            true
        }
    }
}

pub fn handle_events(
    events_to_apply: Vec<Event>,
    subgame: &mut play::Game,
    selected_index: &mut usize,
    context_variables: &mut HashMap<String, String>,
    undo_stack: &mut Vec<Step>,
    redo_stack: &mut Vec<Step>,
    music_maker: &mut MusicMaker,
) {
    for event in events_to_apply {
        log::debug!("Event: {}", event);

        let back_event = match &event {
            Event::AddMember { .. } => Event::RemoveMember {
                index: subgame.members.len(),
            },
            Event::RemoveMember { index } => Event::AddMember {
                index: Some(*index),
                member: subgame.members[*index].to_owned(),
            },
            Event::MoveMember { index, from, to } => Event::MoveMember {
                index: *index,
                from: *to,
                to: *from,
            },
            Event::RenameMember { index, from, to } => Event::RenameMember {
                index: *index,
                from: to.to_owned(),
                to: from.to_owned(),
            },
            Event::UpdateChore { id, .. } => Event::UpdateChore {
                id: *id,
                chore: Box::new(subgame.members[id.member].todo_list[id.chore].clone()),
            },
            Event::MoveChoreUp { id } => Event::MoveChoreDown { id: *id },
            Event::MoveChoreDown { id } => Event::MoveChoreUp { id: *id },
            Event::MoveQuestionUp { id } => Event::MoveQuestionDown { id: *id },
            Event::MoveQuestionDown { id } => Event::MoveQuestionUp { id: *id },
            Event::MoveDemandUp { id } => Event::MoveDemandDown { id: *id },
            Event::MoveDemandDown { id } => Event::MoveDemandUp { id: *id },
            Event::UpdateQuestion { id, .. } => Event::UpdateQuestion {
                id: *id,
                question: subgame.members[id.member].todo_list[id.chore].questions[id.question]
                    .clone(),
            },
            Event::UpdateDemand { id, .. } => Event::UpdateDemand {
                id: *id,
                demand: subgame.members[id.member].todo_list[id.chore].demands[id.demand].clone(),
            },
            Event::AddCharacter { index, ch: _ch } => Event::RemoveCharacter { index: *index },
            Event::RemoveCharacter { index } => Event::AddCharacter {
                index: *index,
                // TODO: \\ was warning letter
                ch: subgame.members[*index]
                    .text
                    .contents
                    .chars()
                    .last()
                    .unwrap_or('\\'),
            },
            Event::SetStartSprite { index, from, to } => Event::SetStartSprite {
                index: *index,
                from: *to,
                to: *from,
            },
            Event::AddNote {
                editing_position,
                note,
            } => {
                if let Some(old) = music_maker.notes().iter().find(|n| n.offset == note.offset) {
                    Event::AddNote {
                        editing_position: *editing_position,
                        note: *old,
                    }
                } else {
                    log::debug!("REMOVE THAT");
                    Event::RemoveNote {
                        editing_position: *editing_position,
                        note: *note,
                    }
                }
            }
            Event::RemoveNote {
                editing_position,
                note,
            } => Event::AddNote {
                editing_position: *editing_position,
                note: *note,
            },
            Event::SwitchToExtendedKeyboard {
                editing_position,
                old_notes,
            } => Event::SwitchToStandardKeyboard {
                editing_position: *editing_position,
                old_notes: old_notes.clone(),
            },
            Event::SwitchToStandardKeyboard {
                editing_position,
                old_notes,
            } => Event::SwitchToExtendedKeyboard {
                editing_position: *editing_position,
                old_notes: old_notes.clone(),
            },
            Event::SwitchToAlternativeSignature {
                editing_position,
                old_notes,
            } => Event::SwitchToStandardSignature {
                editing_position: *editing_position,
                old_notes: old_notes.clone(),
            },
            Event::SwitchToStandardSignature {
                editing_position,
                old_notes,
            } => Event::SwitchToAlternativeSignature {
                editing_position: *editing_position,
                old_notes: old_notes.clone(),
            },
            Event::SetPixels {
                updates,
                left_to_right,
            } => Event::SetPixels {
                updates: updates.clone(),
                left_to_right: !left_to_right,
            },
        };

        let mut reversed_redo_stack = redo_stack.clone();
        reversed_redo_stack.reverse();

        undo_stack.append(&mut reversed_redo_stack);

        for step in &mut *redo_stack {
            step.direction = !step.direction;
        }

        undo_stack.append(redo_stack);

        /*let mut reversed_redo_stack = redo_stack.clone();
        reversed_redo_stack.reverse();

        undo_stack.append(&mut reversed_redo_stack);

        for step in &mut redo_stack {
            step.direction = !step.direction;
        }
        undo_stack.append(&mut redo_stack);*/

        /* TODO: if wanted to limit undo_stack size:
        let over_200 = undo_stack.len() as i64 - 20;
        if over_200 > 0 {
            undo_stack.drain(0..over_200 as usize);
        }*/

        if apply_event(
            &event,
            &mut subgame.members,
            selected_index,
            context_variables,
            music_maker,
            &mut subgame.assets,
        ) {
            undo_stack.push(Step {
                back: back_event,
                forward: event,
                direction: StepDirection::Forward,
            });
        } else {
            log::debug!("Didn't apply event");
        }
    }
}

fn update_using_sprite_sheet(
    position: Vec2,
    mouse: Mouse,
    context_variables: &mut HashMap<String, String>,
) {
    let sprite = sprite_from_context(context_variables);

    let effective_page_width = page_width_for_sprite(sprite.size);

    let n_per_sheet = match sprite.size {
        SpriteSize::OuterBg => 2,
        SpriteSize::InnerBg => 6,
        SpriteSize::Empty => 0,
        SpriteSize::Square(size) => (512 / size).pow(2),
    };

    let pixel_size = sprite_size_in_pixels(sprite.size);

    let inc = (pixel_size.w / 8) as f32;

    // TODO: Mess 2
    let mut z = 0.0;
    let mut w = 0.0;

    for i in 0..n_per_sheet {
        let start_x = (pixel_size.w * i) % effective_page_width;
        let start_y = ((pixel_size.w * i) / effective_page_width) * pixel_size.h;
        let was_new_thingy = if i == 0 {
            false
        } else {
            start_y != ((pixel_size.w * (i - 1)) / effective_page_width) * pixel_size.h
        };

        if was_new_thingy {
            z = 0.0;
            w += inc;
        }
        let x = start_x as f32 / 4.0 + z + position.x - 96.0;
        let y = start_y as f32 / 4.0 + w + position.y - 96.0;
        z += inc;

        let mouse_position = mouse.position;
        if mouse_position.x >= x as i32
            && mouse_position.x < x as i32 + pixel_size.w as i32 / 4
            && mouse_position.y >= y as i32
            && mouse_position.y < y as i32 + pixel_size.h as i32 / 4
            && mouse.left_button.is_pressed()
        {
            context_variables.insert("Sprite Index".to_owned(), i.to_string());
        }
    }
}

async fn apply_menu_actions(
    menu_actions: Vec<menu::Action>,
    editor: &mut Editor,
    environment: &mut Environment,
    navigation: &mut Navigation,
    game: &mut play::Game,
    subgame: &mut play::Game,
    audio_player: &mut AudioPlayer,
    transition: &mut Transition,
) -> WhyResult<MenuOutcome> {
    // TODO: Use hashset? But then don't know order, except imposed order
    for menu_action in menu_actions {
        match menu_action {
            // TODO: These these Play/Pause/Stop up with a nice interface
            menu::Action::Play => {
                if let Some(inner_game) = editor.paused_copy.take() {
                    // *subgame = inner_game;
                    editor.inner_copy = Some(inner_game.clone());
                } else if let Some(inner_game) = editor.inner_copy.take() {
                    *subgame = inner_game;
                    editor.inner_copy = Some(subgame.clone());
                } else {
                    editor.inner_copy = Some(subgame.clone());
                }
                subgame.frame_number = 0;
                log::debug!("Play!");
                audio_player.play_music(subgame.assets.music_data.clone())?;
            }
            menu::Action::Pause => {
                /*if let Some(inner_game) = editor.inner_copy.take() {
                    *subgame = inner_game;
                }*/
                if let Some(inner_game) = editor.inner_copy.take() {
                    editor.paused_copy = Some(inner_game.clone());
                    //editor.inner_copy = Some(inner_game.clone());
                }
                if let Some(sink_player) = &mut audio_player.sink_player {
                    sink_player.music_sink.pause();
                    sink_player.sfx_sinks.clear();
                }
                log::debug!("Stop!");
            }
            menu::Action::Stop => {
                if let Some(inner_game) = editor.paused_copy.take() {
                    *subgame = inner_game;
                } else if let Some(inner_game) = editor.inner_copy.take() {
                    *subgame = inner_game;
                }
                if let Some(sink_player) = &mut audio_player.sink_player {
                    sink_player.music_sink.stop();
                    sink_player.sfx_sinks.clear();
                }
                log::debug!("Stop!");
            }
            menu::Action::Quit => {
                log::debug!("Exiting from Quit menu action");
                return Ok(MenuOutcome::Quit);
            }
            menu::Action::MoveToGame { name } => {
                // TODO: Loading new game should happen after drawing
                log::debug!("Move to Game! {}", name);
                // TODO: Like a cartridge reset?

                //game = temp_load("Green", &name).await?;
                navigation.queue.index += 1;
                navigation.queue.links.truncate(navigation.queue.index);

                let collection = &environment.context["Collection"];
                navigation.queue.links.push(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });

                navigation.next_game = Some(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });
            }
            menu::Action::FadeToGame { name } => {
                // TODO: Loading new game should happen after drawing
                log::debug!("Fade to Game! {}", name);
                // TODO: Like a cartridge reset?
                //if name != "Frog" {
                *transition = Transition::FadeIn {
                    game: game.clone(),
                    fade_left: FADE_LEN,
                };

                navigation.queue.index += 1;
                navigation.queue.links.truncate(navigation.queue.index);

                let collection = &environment.context["Collection"];
                navigation.queue.links.push(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });

                navigation.next_game = Some(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });
            }
            menu::Action::FadeOut => {
                navigation.queue.index = navigation.queue.index.max(1) - 1;
                let name = &navigation.queue.links[navigation.queue.index].game;
                // TODO: Work thsi out, shouldn't temp_load because do that later
                *game = temp_load("Green", name).await?;
                log::debug!("FADING OUT TO: {}", name);

                *transition = Transition::FadeOut {
                    game: game.clone(),
                    fade_left: FADE_LEN,
                };

                audio_player.play_music(game.assets.music_data.clone())?;

                let collection = &environment.context["Collection"];
                navigation.next_game = Some(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });
            }
            menu::Action::BackInQueue => {
                navigation.queue.index = navigation.queue.index.max(1) - 1;
                let name = &navigation.queue.links[navigation.queue.index].game;
                //game = temp_load("Green", name).await?;

                let collection = &environment.context["Collection"];
                navigation.next_game = Some(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });
            }
            menu::Action::NextInQueue => {
                // TODO: Check bounds
                navigation.queue.index += 1;
                if navigation.queue.index >= navigation.queue.links.len() {
                    log::warn!("Looping back around queue index");
                    navigation.queue.index = 0;
                    navigation.queue.links.truncate(1);
                }
                let name = &navigation.queue.links[navigation.queue.index].game;
                // TODO: Frame number and stuff, resetting everything
                log::debug!("NEXT IN QUEUE: {:?}", name);
                //game = temp_load("Green", name).await?;

                let collection = &environment.context["Collection"];
                navigation.next_game = Some(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });
            }
            menu::Action::AddToQueue { name } => {
                let collection = &environment.context["Collection"];
                navigation.queue.links.push(Link {
                    collection: collection.to_owned(),
                    game: name.to_owned(),
                });
                log::debug!("ADDED: {:?}", navigation.queue.links);
            }
            menu::Action::ResetQueue => {
                log::debug!("BEFORE: {:?}", navigation.queue.links);
                navigation.queue.links.truncate(navigation.queue.index + 1);
                log::debug!("AFTER: {:?}", navigation.queue.links);
            }
            menu::Action::New => {
                let bytes = bytes_from_dir("images", DEFAULT_IMAGE_FILENAME)
                    .await
                    .unwrap();
                let image_string = ImageString(BaseEncoder.encode(&bytes));

                let bytes = bytes_from_dir("fonts", DEFAULT_FONT_FILENAME)
                    .await
                    .unwrap();
                let font_string = ImageString(BaseEncoder.encode(&bytes));

                let size = get_typed_variable(&environment.context, "Game Size").unwrap();
                let rng = SeededRng::new(macroquad::miniquad::date::now() as _);

                *subgame = game_from_cartridge(
                    serial::Cartridge::new(size, image_string, font_string),
                    rng,
                );
                editor.selected_index = 0;
                editor.index_tracker = 0;
                editor.previous_hovered_indices = Vec::new();
            }
            menu::Action::Load => {
                let game_filename = environment
                    .context
                    .get("Game File Name")
                    .unwrap()
                    .to_string();
                log::debug!("Loading: {}", game_filename);
                // TODO: Use sub collection variable or something?
                *subgame = temp_load(&environment.context["Collection"], &game_filename).await?;
                editor.selected_index = 0;
                editor.index_tracker = 0;
                editor.previous_hovered_indices = Vec::new();

                environment.update_var(
                    "Image",
                    subgame
                        .assets
                        .filenames
                        .image
                        .clone()
                        .unwrap_or_else(|| "DEBUGEMPTY".to_string()),
                );

                environment.update_var_as_debug("Game Size", subgame.size);
                environment.update_var_as_debug("Length", subgame.length);
                environment.update_var("Game", game_filename);
            }
            menu::Action::Save => {
                temp_save(
                    &environment.context["Collection"],
                    &environment.context["Game"],
                    subgame.clone(),
                )?;
            }
            menu::Action::SetImageFile => {
                let image_filename = &environment.context["Image File Name"];
                let image_filename = format!("{}.png", image_filename);
                let bytes = bytes_from_dir("images", &image_filename).await.unwrap();
                subgame.assets.image_string = ImageString(BaseEncoder.encode(&bytes));

                subgame.assets.texture = texture_from_bytes(&bytes).unwrap();
                subgame.assets.image = subgame.assets.texture.get_texture_data();

                subgame.assets.filenames.image = Some(image_filename.to_owned());
            }
            menu::Action::SetMusicFile => {
                let music_filename = &environment.context["Music File Name"];
                let music_filename = format!("{}.ogg", music_filename);
                let bytes = bytes_from_dir("music", &music_filename).await.unwrap();
                subgame.assets.music_string = Some(SoundString(BaseEncoder.encode(&bytes)));

                subgame.assets.music_data = Some(bytes);

                subgame.assets.filenames.music = Some(music_filename.to_owned());
            }
            menu::Action::PreviewMusic => {
                let music_filename = &environment.context["Music File Name"];
                let music_filename = format!("{}.ogg", music_filename);
                let preview_music = bytes_from_dir("music", &music_filename).await.unwrap();
                audio_player.play_music(Some(preview_music))?;
            }
            menu::Action::StopMusic => {
                audio_player.stop_music();
            }
        }
    }

    Ok(MenuOutcome::None)
}

async fn bytes_from_dir(dir: &str, filename: &str) -> WhyResult<Vec<u8>> {
    let bytes = macroquad::file::load_file(&format!("{}/{}", dir, filename)).await?;
    Ok(bytes)
}
