#![allow(clippy::comparison_chain)]
#![allow(clippy::approx_constant)]

const TEMP_TESTING_INTRO_TEXT: bool = false;

// TODO: EACH NEW GAME IS LIKE A COMPLETE CARTRIDGE RESET
// TODO: Download mq js bundle and store it locally
// TODO: ResetQueue when doing this stuff?
// TODO: .png at end of Image?
// TODO: Fade in/out should take N ticks not an uneven number
// TODO: Time End for Frog and White fade outs not 4-12...
// Vec2(117.0, 32.0) to Vec2(117.0, 32.0)
// TODO: Don't change clicked selected sprite if higher layer sprite in outer game clicked
// TODO: Outer games can be games too
// TODO: If play game, load new game, then stop game, the old game comes back
// TODO: Different intro texts for different difficulties
// TODO: Can you move members (with middle click or move mode) if {Screen} isn't a member?
// TODO: Have mouse_in_world/camera handle scale
// TODO: Restrict setting context variables in subgame (previews?) dummy context?
// TODO: Don't put draw_tool in metagame?
// TODO: Separate page numbers for different variables?
// TODO: Render to RenderTarget texture instead of image in Draw, check performance
// TODO: Custom brushes with random chance?
// TODO: Mixing black colours in piano notes
// TODO: Art animate -> starts off with no entries, if click add looks like nothing was added
// TODO: Save before crashing
// TODO: Make it so animation has to have at least 1 frame, and stop deleting last frame

use macroquad::{
    color::{colors as quad_colours, Color as Colour},
    experimental::coroutines::{start_coroutine, Coroutine},
    input::KeyCode,
    logging as log,
    miniquad::conf::Icon,
    texture::FilterMode,
    window::{next_frame, Conf},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ffi::OsStr, sync::mpsc};

use crate::{
    aud::{default_smf, AudioParameters, AudioPlayer, MidiFileContainer},
    doodle::DrawTool,
    drawer::Camera,
    edit::{AssetChoices, Editor, ImageChoice},
    err::WhyResult,
    inp::{pressed_chars, Input, Mouse, RepeatableButton},
    maths::Vec2,
    meta::{
        update_metagame, Environment, MenuOutcome, Transition, INITIAL_SCALE, INTRO_FONT_FILENAME,
        OUTER_CENTRE, OUTER_HEIGHT, OUTER_WIDTH, PLAY_SCREEN_NAME,
    },
    music::{MakerNote, MusicMaker},
    nav::{Link, Navigation},
    play::{cartridge_from_game, game_from_cartridge, position_in_world, DifficultyLevel},
    rend::{Image, Texture},
    seeded_rng::SeededRng,
    serial::Cartridge,
    time::TimeKeeping,
    whydraw::WhyDrawer,
};

mod anim;
mod art;
mod aud;
mod coll;
mod colours;
mod common;
mod doodle;
mod drawer;
mod edit;
mod err;
mod history;
mod inp;
mod maths;
mod menu;
mod meta;
mod music;
mod nav;
mod pixels;
mod play;
mod rend;
mod seeded_rng;
mod serial;
mod sys;
mod time;
mod track;
mod whydraw;
mod window;

const INITIAL_FULLSCREEN: bool = false;
const INITIAL_WINDOW_WIDTH: u32 = (OUTER_WIDTH as f32 * INITIAL_SCALE) as u32;
const INITIAL_WINDOW_HEIGHT: u32 = (OUTER_HEIGHT as f32 * INITIAL_SCALE) as u32;
const INITIAL_WINDOW_SIZE: pixels::Size =
    pixels::Size::new(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT);

const IS_RECORDING_FPS: bool = false;

//const DEFAULT_FONT_FILENAME: &'static str = "analog.png";

impl coll::Grid for Image {
    fn size(&self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }

    fn get_square_bit(&self, position: pixels::Position) -> bool {
        self.get_pixel(position.x as u32, position.y as u32).a != 0.0
    }
}

#[cfg(test)]
mod tests {
    use crate::doodle::PAINT_SIZE;
    use crate::pixels::Position;
    use crate::pixels::Rect as SecRect;

    use super::coll::{Grid, _is_subsection_square_active};
    use super::*;

    #[test]
    fn test_grid() {
        #[rustfmt::skip]
        let grid = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, colours::BLANK);

        assert!(!grid.is_square_active(Position::new(0, 0)));
    }

    #[test]
    fn test_outside_grid() {
        #[rustfmt::skip]
        let grid = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, colours::BLANK);

        assert!(!grid.is_square_active(Position::new(-5, -5)));
    }

    #[test]
    fn test_subsection() {
        #[rustfmt::skip]
        let mut grid = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, colours::BLANK);
        grid.set_pixel(1, 1, colours::WHITE);

        let section = SecRect::aabb(1, 1, 3, 3);
        assert!(_is_subsection_square_active(
            &grid,
            Position::new(0, 0),
            section
        ));
        assert!(!_is_subsection_square_active(
            &grid,
            Position::new(1, 0),
            section
        ));
    }
}

pub async fn temp_load(collection: &str, name: &str) -> WhyResult<play::Game> {
    log::debug!("game name: {}/{}", collection, name);
    let filename = format!("collections/{}/{}.json", collection, name);
    let save_contents = macroquad::file::load_string(&filename).await?;
    let cartridge: Result<Cartridge, _> = serde_json::from_str(&save_contents);

    let rng = SeededRng::new(macroquad::miniquad::date::now() as _);

    cartridge
        .map(|cart| game_from_cartridge(cart, rng))
        .map_err(|e| format!("Error deserialising game: {:?}", e).into())
}

pub fn temp_save(collection: &str, name: &str, game: play::Game) -> WhyResult<()> {
    log::debug!("write game name: {}/{}", collection, name);
    let filename = format!("collections/{}/{}.json", collection, name);
    let cartridge = cartridge_from_game(game);
    let s = serde_json::to_string_pretty(&cartridge).unwrap();
    std::fs::write(filename, s)?;
    Ok(())
}

async fn system_texture(filename: &str) -> WhyResult<Texture> {
    texture_from_dir("system", filename).await
}

async fn texture_from_dir(dir: &str, filename: &str) -> WhyResult<Texture> {
    let texture = macroquad::texture::load_texture(&format!("{}/{}", dir, filename)).await?;
    texture.set_filter(FilterMode::Nearest);
    Ok(texture)
}

fn texture_from_bytes(bytes: &[u8]) -> WhyResult<Texture> {
    let texture = Texture::from_file_with_format(bytes, None);
    texture.set_filter(FilterMode::Nearest);
    Ok(texture)
}

async fn images_texture(filename: &str) -> WhyResult<Texture> {
    texture_from_dir("images", filename).await
}

fn load_icon_to_array(bytes: &'static [u8], out: &mut [u8]) {
    let image = Image::from_file_with_format(bytes, None).unwrap();
    let mut i = 0;
    for pixel in image.get_image_data() {
        for colour in pixel.iter() {
            out[i] = *colour;
            i += 1;
        }
    }
}

fn window_conf() -> Conf {
    let mut icon_16x: [u8; 1024] = [0; 1024];
    let mut icon_32x: [u8; 4096] = [0; 4096];
    let mut icon_64x: [u8; 16384] = [0; 16384];

    load_icon_to_array(include_bytes!("../system/icon16.png"), &mut icon_16x);
    load_icon_to_array(include_bytes!("../system/icon32.png"), &mut icon_32x);
    load_icon_to_array(include_bytes!("../system/icon64.png"), &mut icon_64x);

    let custom_icon = Icon {
        small: icon_16x,
        medium: icon_32x,
        big: icon_64x,
    };

    Conf {
        window_title: "whygames (Green)".to_owned(),
        fullscreen: INITIAL_FULLSCREEN,
        window_width: INITIAL_WINDOW_WIDTH as i32,
        window_height: INITIAL_WINDOW_HEIGHT as i32,
        icon: Some(custom_icon),
        ..Default::default()
    }
}

#[cfg(target_arch = "wasm32")]
use sapp_jsutils::JsObject;

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn hi_from_wasm(x: JsObject);
}

fn rng_from_time() -> SeededRng {
    SeededRng::new(macroquad::miniquad::date::now() as u64)
}

#[derive(Serialize, Deserialize)]
struct BootInfo {
    initial_game: Link,
    initial_subgame: Link,
}

#[macroquad::main(window_conf)]
async fn main() -> WhyResult<()> {
    log::info!("Whygames 0.1");

    macroquad::input::prevent_quit();

    let mut window = window::Tracker::new(INITIAL_FULLSCREEN, INITIAL_WINDOW_SIZE);

    let mut transition = Transition::None;

    let mut environment = {
        let context: HashMap<String, String> = {
            let s = macroquad::file::load_string("system/context.json").await?;
            serde_json::from_str(&s)?
        };

        Environment {
            score: 0,
            difficulty_level: DifficultyLevel::default(),
            context,
            rng: rng_from_time(),
        }
    };

    let boot_info: BootInfo = {
        let s = macroquad::file::load_string("system/conf.json").await?;
        serde_json::from_str(&s)?
    };
    let mut draw_tool = DrawTool::init();
    let mut music_maker = MusicMaker::init();

    let mut game = play::Game::load(&boot_info.initial_game).await?;
    let mut subgame = play::Game::load(&boot_info.initial_subgame).await?;

    environment.init_vars(&subgame, &boot_info);

    let mut navigation = Navigation::new(boot_info.initial_game);

    // TODO: Careful when actually running on wasm
    #[cfg(target_arch = "wasm32")]
    {
        subgame.assets.image = Image::gen_image_color(
            meta::INNER_WIDTH as u16,
            meta::INNER_HEIGHT as u16,
            colours::WHITE,
        );

        //subgame.assets.texture.update(&subgame.assets.image);
        subgame.assets.texture = macroquad::texture::Texture2D::from_image(&subgame.assets.image);
        subgame.assets.texture.set_filter(FilterMode::Nearest);
    }

    let mut inner_camera = match subgame.size {
        play::Size::Small => Camera::Inner {
            position: game.screen_position(),
        },
        play::Size::Big => Camera::EditedOuter {
            position: game.screen_position(),
        },
    };
    let mut input = {
        let original_mouse_position = {
            let (x, y) = macroquad::input::mouse_position();
            pixels::Position::new(x as i32, y as i32)
        };
        let outer_mouse = Mouse {
            position: position_in_world(original_mouse_position, Camera::Outer),
            ..Default::default()
        };

        let inner_mouse = Mouse {
            position: position_in_world(original_mouse_position, inner_camera),
            ..Default::default()
        };
        let keyboard = HashMap::from([
            (KeyCode::Z, RepeatableButton::default()),
            (KeyCode::Y, RepeatableButton::default()),
        ]);
        Input {
            outer: outer_mouse,
            inner: inner_mouse,
            rmb_held_down_for: 0,
            chars_pressed: pressed_chars(),
            mouse_scroll: 0.0,
            keyboard,
        }
    };

    let sf2_data = {
        let (tx, rx) = mpsc::channel();

        let resources_loading: Coroutine = start_coroutine(async move {
            let sf2_data = macroquad::file::load_file("why.sf2");

            tx.send(sf2_data.await.unwrap()).unwrap();
        });

        let mut counter = 1;
        while !resources_loading.is_done() {
            counter += 1;
            let dots = (counter / 15) % 3 + 1;
            let dots = ".".repeat(dots);
            macroquad::text::draw_text(
                &format!("Loading{}", dots),
                20.0,
                20.0,
                30.0,
                quad_colours::WHITE,
            );

            next_frame().await;
        }

        rx.recv()
    }?;

    let mut audio_player = {
        let audio_params = AudioParameters::load("system/audio_params.json").await?;

        AudioPlayer::init(audio_params, sf2_data).await?
    };

    audio_player.play_music(game.assets.music_data.clone())?;
    audio_player.switch_to_maker_instrument(&music_maker);

    //let synth_temp = synthesizer.clone();

    let mut smf = default_smf();

    let mut midi_file_container = MidiFileContainer::new(&smf)?;

    let mut fps_record = vec![0; 60];

    let mut editor = {
        // TODO: Wasm? Is wasm editor a goal?
        let mut image_file_choices = Vec::new();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let paths = std::fs::read_dir("./images").unwrap();
            for path in paths {
                let p = path.unwrap().path();
                let file_stem = p.file_stem().unwrap().to_str().unwrap();

                if p.extension() == Some(OsStr::new("png")) {
                    image_file_choices.push(ImageChoice {
                        name: file_stem.to_string(),
                        texture: None,
                    });
                }
            }
        }

        let mut music_file_choices = Vec::new();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let paths = std::fs::read_dir("./music").unwrap();
            for path in paths {
                let p = path.unwrap().path();
                let file_stem = p.file_stem().unwrap().to_str().unwrap();

                music_file_choices.push(file_stem.to_string());
            }
        }

        log::debug!("{:?}", music_file_choices);

        let mut game_file_choices = Vec::new();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let paths = std::fs::read_dir("./collections/Green").unwrap();
            for path in paths {
                let p = path.unwrap().path();
                let file_stem = p.file_stem().unwrap().to_str().unwrap();

                game_file_choices.push(file_stem.to_string());
            }
        }

        Editor {
            selected_index: 0,
            page: 0,
            edit_text_index: None,
            choices: AssetChoices {
                games: game_file_choices,
                images: image_file_choices,
                music: music_file_choices,
            },
            ..Default::default()
        }
    };

    let mut dummy_editor = Editor::default();

    let mut drawer = WhyDrawer::init().await?;

    // Makes first frame delay shorter
    next_frame().await;

    // Ensure this is last before loop
    let mut time_keeping = TimeKeeping::new(macroquad::time::get_time());

    'main_loop: loop {
        if IS_RECORDING_FPS {
            fps_record.remove(0);
            fps_record.push(macroquad::time::get_fps());
            macroquad::ui::root_ui().label(
                Vec2::new(0.0, 0.0),
                &format!("{}", fps_record.iter().sum::<i32>() / 60),
            );
            macroquad::ui::root_ui().label(
                Vec2::new(0.0, 20.0),
                &format!("{:?}", input.outer.left_button),
            );
        }

        time_keeping.update(macroquad::time::get_time());

        inner_camera = match subgame.size {
            play::Size::Small => Camera::Inner {
                position: game.screen_position(),
            },
            // TODO: ?
            play::Size::Big => {
                if game
                    .members
                    .iter()
                    .any(|m| m.text.contents == PLAY_SCREEN_NAME)
                {
                    Camera::Outer
                } else {
                    Camera::EditedOuter {
                        position: OUTER_CENTRE,
                    }
                }
            }
        };

        // TODO: Unoptimised, maybe don't worry
        environment.update_var(
            "Selected Member",
            subgame.members[editor.selected_index].name.to_owned(),
        );

        // TODO: Have this happen in input.update() but still be smooth
        input.mouse_scroll = macroquad::input::mouse_wheel().1;
        music_maker.update_note_length(&mut environment, input.mouse_scroll);

        'multi_loop: while time_keeping.has_more_frames_to_play(game.frame_number) {
            input.update(inner_camera, &mut draw_tool.tracker.temp_save);

            let outcome = update_metagame(
                &mut environment,
                &mut navigation,
                (&mut editor, &mut dummy_editor),
                &mut draw_tool,
                &mut music_maker,
                &mut game,
                &mut subgame,
                &input,
                &mut audio_player,
                &mut transition,
                time_keeping,
            )
            .await?;

            if game.music_maker_member().is_some() {
                music_maker
                    .handle_actions(
                        &mut midi_file_container,
                        &mut smf,
                        &mut audio_player,
                        &game,
                        &drawer.music_image,
                        &input,
                    )
                    .ok();
                // ok() for now
            }

            if let MenuOutcome::Quit = outcome {
                break 'main_loop;
            }

            if navigation.next_game.is_some() {
                break 'multi_loop;
            }
        }

        drawer.init_frame();

        drawer
            .draw_metagame(
                &environment,
                &navigation,
                &draw_tool,
                &music_maker,
                inner_camera,
                &game,
                &subgame,
                &mut editor,
                &input,
                &mut transition,
                &audio_player,
            )
            .await;

        if let Some(link) = navigation.next_game.take() {
            time_keeping.reset();
            game = temp_load(&link.collection, &link.game).await?;
            game.frame_number = 0;
            log::debug!("FRAME NUMBER: {}", game.frame_number);
            // TODO: Think about if this is what we want all the time
            //if let Some(playing_game) = inner_copy.take() {
            //subgame = playing_game;
            //}
        }

        if macroquad::prelude::is_key_down(KeyCode::LeftAlt)
            && (macroquad::prelude::is_key_pressed(KeyCode::Enter)
                || macroquad::prelude::is_key_pressed(KeyCode::KpEnter))
        {
            window.placement = !window.placement;

            if window.placement == window::Placement::Fullscreen {
                let last_window_size = {
                    let w = macroquad::window::screen_width() as u32;
                    let h = macroquad::window::screen_height() as u32;
                    pixels::Size { w, h }
                };
                window.last_size = last_window_size;
            }

            macroquad::window::set_fullscreen(window.placement.is_fullscreen());

            if window.placement == window::Placement::Windowed {
                let pixels::Size { w, h } = window.last_size;
                macroquad::window::request_new_screen_size(w as f32, h as f32);
            }
        }

        {
            // TODO: Tidy up
            let w = macroquad::window::screen_width();
            let h = macroquad::window::screen_height();
            // FIxes thing with window size getting smaller if minimised
            if w > 1.0 || h > 1.0 {
                if w < OUTER_WIDTH as f32 {
                    macroquad::window::request_new_screen_size(OUTER_WIDTH as f32, h);
                }
                if h < OUTER_HEIGHT as f32 {
                    macroquad::window::request_new_screen_size(w, OUTER_HEIGHT as f32);
                }
                if w < OUTER_WIDTH as f32 && h < OUTER_HEIGHT as f32 {
                    macroquad::window::request_new_screen_size(
                        OUTER_WIDTH as f32,
                        OUTER_HEIGHT as f32,
                    );
                }
            }
        }

        if macroquad::input::is_quit_requested() {
            break;
        }

        next_frame().await;
    }

    log::debug!("Quitting");

    Ok(())
}
