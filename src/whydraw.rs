use super::images_texture;
use crate::anim::animation_time_from_speed;
use crate::art::{Sprite, SpriteSize};
use crate::aud::AudioPlayer;
use crate::coll::CollisionObject;
use crate::colours;
use crate::doodle::{DrawTool, PreviewShape, Shape, ShapeStyle};
use crate::drawer::{
    drawn_from_top_left, drawn_rect, drawn_sized_rect, drawn_source_rect, drawn_square,
    page_width_for_sprite, sheet_source_rect, sprite_size_in_pixels, Camera, DrawParams, Drawer,
    FancyText,
};
use crate::err::WhyResult;
use crate::music::{
    is_outside_edge, note_positions_draw, octave_height, rough_octave_span, rough_offset,
    sprite_offset, MakerNote, MusicMaker, RelativeNote,
};
use crate::nav::Navigation;
use crate::system_texture;
use std::collections::HashMap;

use crate::edit::{
    demand_from_context, fancy_demand_text, fancy_demand_text_for_list, fancy_question_text,
    fancy_question_text_for_chore, fancy_question_text_for_list, general_area_size,
    get_typed_variable, index_from_member_text, is_position_in_general_area, max_var_per_page,
    offset_for_page, padded_len, question_from_context, simple_text, sprite_from_context, Editor,
    Fancy,
};

use crate::inp::{Input, Mouse};
use crate::meta::{
    is_screen_member_name, Environment, Transition, CHOOSE_AREA_NAME, CHOOSE_POINT_NAME,
    EDITABLE_SCREEN_NAME, FADE_LEN, INNER_HEIGHT, INNER_WIDTH, MUSIC_MAKER_NAME, OUTER_CENTRE,
};
use crate::pixels::{self, Position};
use crate::play::{self, is_position_in_sprite_sheet_image};
use crate::serial::IntroText;
use crate::TEMP_TESTING_INTRO_TEXT;
use macroquad::{
    color::{colors as quad_colours, Color as Colour},
    input::KeyCode,
    math::Vec2,
    texture::{FilterMode, Image, Texture2D},
    window::{self as quad_window},
};
use regex::Regex;

// TODO: Make private
pub struct Regexes {
    pub member_preview: Regex,
    pub image_file: Regex,
    pub sprite: Regex,
    pub music_name: Regex,
    pub game_name: Regex,
    pub member_name: Regex,
    pub paint: Regex,
}

pub struct WhyDrawer {
    pub drawer: Drawer,
    pub base_texture: Texture2D,
    pub box_texture: Texture2D,
    pub music_texture: Texture2D,
    pub ins_texture: Texture2D,
    pub music_image: Image,
    pub intro_font: play::BitmapFont,
    pub regexes: Regexes,
}

impl WhyDrawer {
    pub async fn init() -> WhyResult<WhyDrawer> {
        let intro_font = {
            let texture = system_texture(crate::INTRO_FONT_FILENAME).await?;
            play::BitmapFont::new(texture)
        };

        let box_texture = system_texture("eyes.png").await.unwrap();
        let base_texture = system_texture("base.png").await.unwrap();
        let music_texture = system_texture("music-texture.png").await.unwrap();
        let ins_texture = system_texture("ins1.png").await.unwrap();

        let regexes = Regexes {
            member_preview: Regex::new(r"\{Member Preview (\d*)\}").unwrap(),
            image_file: Regex::new(r"\{Image File (\d*)\}").unwrap(),
            sprite: Regex::new(r"\{Sprite (\d*)\}").unwrap(),
            music_name: Regex::new(r"\{Music Name (\d*)\}").unwrap(),
            game_name: Regex::new(r"\{Game Name (\d*)\}").unwrap(),
            member_name: Regex::new(r"\{Member Name (\d*)\}").unwrap(),
            paint: Regex::new(r"\{Paint (\d*)\}").unwrap(),
        };

        Ok(WhyDrawer {
            drawer: Drawer {
                camera: None,
                scale: 1.0,
            },
            intro_font,
            base_texture,
            box_texture,
            music_image: music_texture.get_texture_data(),
            music_texture,
            ins_texture,
            regexes,
        })
    }

    pub fn init_frame(&mut self) {
        self.drawer.clear_window();
        self.drawer.draw_texture(
            Camera::Outer,
            drawn_rect(OUTER_CENTRE.into(), &self.box_texture),
            &self.box_texture,
        );

        let screen_width = quad_window::screen_width();
        let screen_height = quad_window::screen_height();
        let ratio = screen_width / screen_height;
        let intended_ratio = 16.0 / 9.0;

        if ratio > intended_ratio {
            let game_area_rendered_width = screen_height * intended_ratio;
            let blank_space = screen_width - game_area_rendered_width;
            let letterbox_width = blank_space / 2.0;
            unsafe {
                let gl = quad_window::get_internal_gl().quad_gl;
                gl.scissor(Some((
                    letterbox_width as i32,
                    0,
                    (screen_width - blank_space) as i32,
                    screen_height as i32,
                )));
            }
        } else if ratio < intended_ratio {
            let game_area_rendered_height = screen_width / intended_ratio;
            let blank_space = screen_height - game_area_rendered_height;
            let letterbox_height = blank_space / 2.0;
            unsafe {
                let gl = quad_window::get_internal_gl().quad_gl;
                gl.scissor(Some((
                    0,
                    letterbox_height as i32,
                    screen_width as i32,
                    (screen_height - blank_space) as i32,
                )));
            }
        }

        self.drawer.clear(Camera::Outer, colours::WHITE);
    }

    // TODO: Don't load textures here?
    // TODO: Mixing too much render and update code
    pub async fn draw_metagame(
        &mut self,
        environment: &Environment,
        navigation: &Navigation,
        draw_tool: &DrawTool,
        music_maker: &MusicMaker,
        inner_camera: Camera,
        game: &play::Game,
        subgame: &play::Game,
        editor: &mut Editor,
        input: &Input,
        transition: &mut Transition,
        // For drawing purple line...
        audio_player: &AudioPlayer,
    ) {
        for (member_index, member) in game.members.iter().enumerate() {
            let source = sheet_source_rect(member.sprite);
            let params = DrawParams {
                source: Some(source),
                ..Default::default()
            };

            self.drawer.draw_params_texture(
                Camera::Outer,
                drawn_source_rect(member.position, source),
                &game.assets.texture,
                params,
            );

            let mut should_disregard_text = false;

            let re = &self.regexes.member_preview;
            if let Some(i) = index_from_member_text(re, &member.text.contents) {
                let max_per_page = max_var_per_page(&game.members, "Member Preview");
                let len = padded_len(subgame.members.len(), max_per_page);
                let offset = offset_for_page(editor.page, max_per_page, len);

                should_disregard_text = true;

                // TODO: Member Page
                if let Some(sprite) = subgame
                    .members
                    .get(offset + i - 1)
                    .map(|member| member.sprite)
                {
                    let w = 32;
                    let params = DrawParams {
                        source: Some(sheet_source_rect(sprite)),
                        ..Default::default()
                    };

                    if sprite.size != SpriteSize::Empty {
                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_square(member.position, w),
                            &subgame.assets.texture,
                            params,
                        );
                    }
                }
            }

            let re = &self.regexes.image_file;
            if let Some(i) = index_from_member_text(re, &member.text.contents) {
                let max_per_page = max_var_per_page(&game.members, "Image File");
                let len = padded_len(editor.choices.images.len(), max_per_page);
                let offset = offset_for_page(editor.page, max_per_page, len);

                should_disregard_text = true;
                let image_index = i - 1 + offset;
                if image_index < editor.choices.images.len() {
                    if editor.choices.images[image_index].texture.is_none() {
                        let filename = format!("{}.png", editor.choices.images[image_index].name);
                        let texture = images_texture(&filename).await.unwrap();

                        texture.set_filter(FilterMode::Linear);
                        editor.choices.images[image_index].texture = Some(texture);
                    }
                    let texture = editor.choices.images[image_index].texture.as_ref().unwrap();

                    let w = 64;
                    self.drawer.draw_texture(
                        Camera::Outer,
                        drawn_square(member.position, w),
                        texture,
                    );
                }
            }

            let re = &self.regexes.paint;
            if let Some(i) = index_from_member_text(re, &member.text.contents) {
                let max_per_page = max_var_per_page(&game.members, "Paint");
                let len = padded_len(draw_tool.paint_choices.len(), max_per_page);
                let offset = offset_for_page(editor.page, max_per_page, len);

                should_disregard_text = true;

                let paint_index = i - 1 + offset;
                if let Some(texture) = draw_tool
                    .paint_choices
                    .get(paint_index)
                    .map(|paint| &paint.texture)
                {
                    let w = 16;
                    self.drawer.draw_texture(
                        Camera::Outer,
                        drawn_square(member.position, w),
                        texture,
                    );
                }
            }

            if member.text.contents == "{Sprite Sheet}" {
                should_disregard_text = true;

                self.draw_sprite_sheet(
                    member.position,
                    input.outer,
                    &subgame.assets.texture,
                    &environment.context,
                );
            }

            if member.text.contents == "{Image Preview}" {
                should_disregard_text = true;

                let sprite = sprite_from_context(&environment.context);

                let pixel_size = sprite_size_in_pixels(sprite.size);

                if sprite.size != SpriteSize::Empty {
                    const TOOLTIP_IMAGE_SIZE: f32 = 64.0;
                    let max_side = pixel_size.w.max(pixel_size.h) as f32;
                    let dest_size = pixels::Size::new(
                        (pixel_size.w as f32 / max_side * TOOLTIP_IMAGE_SIZE) as u32,
                        (pixel_size.h as f32 / max_side * TOOLTIP_IMAGE_SIZE) as u32,
                    );
                    let params = DrawParams {
                        source: Some(sheet_source_rect(sprite)),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_sized_rect(member.position, dest_size),
                        &subgame.assets.texture,
                        params,
                    );
                }
            }

            if member.text.contents == "{Animation Preview}" {
                should_disregard_text = true;

                if !editor.animation.is_empty() {
                    let speed = get_typed_variable(&environment.context, "Speed").unwrap();
                    let anim_time = animation_time_from_speed(speed);

                    let index = (game.frame_number / anim_time) % editor.animation.len();

                    let sprite = editor.animation[index];

                    let pixel_size = sprite_size_in_pixels(sprite.size);

                    if sprite.size != SpriteSize::Empty {
                        const TOOLTIP_IMAGE_SIZE: f32 = 64.0;
                        let max_side = pixel_size.w.max(pixel_size.h) as f32;
                        let dest_size = pixels::Size::new(
                            (pixel_size.w as f32 / max_side * TOOLTIP_IMAGE_SIZE) as u32,
                            (pixel_size.h as f32 / max_side * TOOLTIP_IMAGE_SIZE) as u32,
                        );
                        let params = DrawParams {
                            source: Some(sheet_source_rect(sprite)),
                            ..Default::default()
                        };

                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_sized_rect(member.position, dest_size),
                            &subgame.assets.texture,
                            params,
                        );
                    }
                }
            }

            if member.text.contents == "{Instrument Pic}" {
                should_disregard_text = true;

                let dest_size = pixels::Size::new(16, 16);
                let sprite = Sprite {
                    index: music_maker.instrument_index() as u32,
                    size: SpriteSize::Square(16),
                };
                let params = DrawParams {
                    source: Some(sheet_source_rect(sprite)),
                    ..Default::default()
                };

                self.drawer.draw_params_texture(
                    Camera::Outer,
                    drawn_sized_rect(member.position, dest_size),
                    &self.ins_texture,
                    params,
                );
            }

            let re = &self.regexes.sprite;
            if let Some(animation_index) = index_from_member_text(re, &member.text.contents) {
                should_disregard_text = true;

                let sprite = editor
                    .animation
                    .get(animation_index - 1)
                    .cloned()
                    .unwrap_or_default();

                let pixel_size = sprite_size_in_pixels(sprite.size);

                if sprite.size != SpriteSize::Empty {
                    const TOOLTIP_IMAGE_SIZE: f32 = 32.0;
                    let max_side = pixel_size.w.max(pixel_size.h) as f32;
                    let dest_size = pixels::Size::new(
                        (pixel_size.w as f32 / max_side * TOOLTIP_IMAGE_SIZE) as u32,
                        (pixel_size.h as f32 / max_side * TOOLTIP_IMAGE_SIZE) as u32,
                    );
                    let params = DrawParams {
                        source: Some(sheet_source_rect(sprite)),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_sized_rect(member.position, dest_size),
                        &subgame.assets.texture,
                        params,
                    );
                }
            }

            if member.text.contents == "{Edit Sprite}" {
                should_disregard_text = true;

                for i in 0..64 {
                    for j in 0..36 {
                        let quart_width = INNER_WIDTH as i32 / 64;
                        let quart_height = INNER_HEIGHT as i32 / 36;
                        let col = if i % 2 == j % 2 {
                            quad_colours::LIGHTGRAY
                        } else {
                            quad_colours::GRAY
                        };

                        self.drawer.draw_rectangle(
                            inner_camera,
                            pixels::Rect::from_top_left(
                                pixels::Position::new(i * quart_width, j * quart_height),
                                pixels::Size::new(quart_width as u32, quart_height as u32),
                            ),
                            col,
                        );
                    }
                }

                let sprite = sprite_from_context(&environment.context);

                if sprite.is_square() {
                    let dest_rect = pixels::Rect::tlwh(56, 0, INNER_HEIGHT, INNER_HEIGHT);
                    self.drawer.draw_grid(inner_camera, dest_rect, 8, 8);
                } else {
                    let dest_rect = pixels::Rect::tlwh(0, 0, INNER_WIDTH, INNER_HEIGHT);
                    self.drawer.draw_grid(inner_camera, dest_rect, 16, 9);
                };

                //self.drawer.clear(inner_camera, quad_colours::BLACK);
                if sprite.is_square() {
                    // TODO: OUTER CAMERA INSTEAD?

                    // Draw border
                    self.drawer.draw_rectangle(
                        inner_camera,
                        pixels::Rect::aabb(0, 0, 56 as i32, INNER_HEIGHT as i32),
                        quad_colours::BLACK,
                    );
                    self.drawer.draw_rectangle(
                        inner_camera,
                        pixels::Rect::aabb(
                            INNER_WIDTH as i32 - 56,
                            0,
                            INNER_WIDTH as i32,
                            INNER_HEIGHT as i32,
                        ),
                        quad_colours::BLACK,
                    );
                }

                if sprite.size != SpriteSize::Empty {
                    //let dest_size = pixels::Size::new(INNER_WIDTH, INNER_HEIGHT);
                    let dest_size = if sprite.is_square() {
                        pixels::Size::new(INNER_HEIGHT, INNER_HEIGHT)
                    } else {
                        pixels::Size::new(INNER_WIDTH, INNER_HEIGHT)
                    };
                    let params = DrawParams {
                        source: Some(sheet_source_rect(sprite)),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_sized_rect(member.position, dest_size),
                        &subgame.assets.texture,
                        params,
                    );
                }

                match draw_tool.tracker.preview_shape {
                    Some(PreviewShape {
                        shape: Shape::Line,
                        style: _style,
                        area,
                    }) => self.drawer.draw_line(
                        inner_camera,
                        area.min,
                        area.max,
                        quad_colours::LIGHTGRAY,
                    ),
                    Some(PreviewShape {
                        shape: Shape::Rectangle,
                        style:
                            ShapeStyle::Lined {
                                thickness: _thickness,
                            },
                        area,
                    }) => {
                        /*self
                        .drawer
                        .draw_rectangle_lines(inner_camera, area, quad_colours::LIGHTGRAY)*/

                        let colour = quad_colours::LIGHTGRAY;
                        self.drawer
                            .draw_line(inner_camera, area.min(), area.top_right(), colour);
                        self.drawer
                            .draw_line(inner_camera, area.min(), area.bottom_left(), colour);
                        self.drawer
                            .draw_line(inner_camera, area.top_right(), area.max(), colour);
                        self.drawer
                            .draw_line(inner_camera, area.bottom_left(), area.max(), colour);
                    }
                    Some(PreviewShape {
                        shape: Shape::Circle,
                        style:
                            ShapeStyle::Lined {
                                thickness: _thickness,
                            },
                        area,
                    }) => self.drawer.draw_ellipse_lines(
                        inner_camera,
                        area,
                        quad_colours::LIGHTGRAY,
                        false,
                    ),
                    Some(PreviewShape {
                        shape: Shape::Rectangle,
                        style: ShapeStyle::Filled,
                        area,
                    }) => self
                        .drawer
                        .draw_rectangle(inner_camera, area, quad_colours::LIGHTGRAY),
                    Some(PreviewShape {
                        shape: Shape::Circle,
                        style: ShapeStyle::Filled,
                        area,
                    }) => self
                        .drawer
                        .draw_ellipse_lines(inner_camera, area, colours::GREY, true),

                    _ => {}
                }
            }

            if !should_disregard_text {
                let mut fancy_text: Vec<FancyText> = simple_text(&member.text.contents);

                if member.text.contents.starts_with("{Chore ") {
                    fancy_text = fancy_question_text_for_chore(member, subgame, editor);
                }

                if member.text.contents == "{Question Stage}" {
                    let question = question_from_context(&environment.context);
                    fancy_text = fancy_question_text(&question);
                    fancy_text.push("?".plain());
                } else if member.text.contents.starts_with("{Question ") {
                    fancy_text =
                        fancy_question_text_for_list(member, subgame, editor, &environment.context);
                }

                if member.text.contents == "{Demand Stage}" {
                    let demand = demand_from_context(&environment.context, &editor.animation);
                    fancy_text = fancy_demand_text(&demand);
                    fancy_text.push("!".plain());
                } else if member.text.contents.starts_with("{Demand ") {
                    fancy_text =
                        fancy_demand_text_for_list(member, subgame, editor, &environment.context);
                }

                // TODO: Location?
                if member.text.contents == "{Score3}" {
                    let mut text_buffer = format!("1{:0>3}", environment.score);
                    text_buffer.insert(2, ':');
                    fancy_text = simple_text(&text_buffer);
                }

                if member.text.contents == "{Instrument Name}" {
                    fancy_text = simple_text(music_maker.current_instrument_name());
                }

                if member.text.contents == "{Track Name}" {
                    fancy_text = simple_text(&format!(
                        "Track {}",
                        music_maker.editing_position.track_index.simple_index() + 1
                    ));
                }

                if member.text.contents == "{Track Index}" {
                    fancy_text = simple_text(&format!(
                        "{}",
                        music_maker.editing_position.track_index.simple_index() + 1
                    ));
                }

                // TODO:
                if member.text.contents.starts_with("{Image") {
                    let max_per_page = max_var_per_page(&game.members, "Image File")
                        .max(max_var_per_page(&game.members, "Image Name"));
                    for i in 1..=max_per_page {
                        let len = padded_len(editor.choices.images.len(), max_per_page);
                        let offset = offset_for_page(editor.page, max_per_page, len);
                        if member.text.contents == format!("{{Image File {}}}", i) {
                            fancy_text = Vec::new();
                        }
                        if member.text.contents == format!("{{Image Name {}}}", i) {
                            let text_buffer = {
                                if let Some(name) =
                                    editor.choices.images.get(i - 1 + offset).map(|f| &f.name)
                                {
                                    name.to_string()
                                } else {
                                    "".to_string()
                                }
                            };
                            fancy_text = simple_text(&text_buffer);
                        }
                    }
                }

                let re = &self.regexes.music_name;
                if let Some(i) = index_from_member_text(re, &member.text.contents) {
                    let max_per_page = max_var_per_page(&game.members, "Music Name");
                    let len = padded_len(editor.choices.music.len(), max_per_page);
                    let offset = offset_for_page(editor.page, max_per_page, len);

                    let text_buffer = {
                        if let Some(name) = editor.choices.music.get(i - 1 + offset) {
                            name.to_string()
                        } else {
                            "".to_string()
                        }
                    };
                    fancy_text = simple_text(&text_buffer);
                }

                let re = &self.regexes.game_name;
                if let Some(i) = index_from_member_text(re, &member.text.contents) {
                    let max_per_page = max_var_per_page(&game.members, "Game Name");

                    let len = padded_len(editor.choices.games.len(), max_per_page);
                    let offset = offset_for_page(editor.page, max_per_page, len);
                    if member.text.contents == format!("{{Game Name {}}}", i) {
                        let text_buffer = {
                            if let Some(name) = editor.choices.games.get(i - 1 + offset) {
                                name.to_string()
                            } else {
                                "".to_string()
                            }
                        };
                        fancy_text = simple_text(&text_buffer);
                    }
                }

                let re = &self.regexes.member_name;
                if let Some(i) = index_from_member_text(re, &member.text.contents) {
                    let max_per_page = max_var_per_page(&game.members, "Member Name");
                    let len = padded_len(subgame.members.len(), max_per_page);
                    let offset = offset_for_page(editor.page, max_per_page, len);
                    let text_buffer = {
                        if let Some(member) = subgame.members.get(i - 1 + offset) {
                            member.name.to_string()
                        } else {
                            "".to_string()
                        }
                    };
                    fancy_text = simple_text(&text_buffer);
                }

                self.drawer.draw_fancy_text(
                    Camera::Outer,
                    member.position,
                    &fancy_text,
                    member.text.colour,
                    &game.assets.font,
                    (&subgame.assets.texture, subgame.size),
                );

                if editor.edit_text_index == Some(member_index) && (game.frame_number / 30) % 2 == 0
                {
                    let position: Vec2 = member.position;
                    let offset = Vec2::new(
                        2.0 + game.assets.font.text_width(&member.text.contents) as f32 / 2.0,
                        0.0,
                    );
                    let screen_rect = pixels::Rect::from_centre(
                        (position + offset).into(),
                        pixels::Size::new(1, game.assets.font.char_height + 2),
                    );
                    self.drawer
                        .draw_rectangle(Camera::Outer, screen_rect, colours::BLACK);
                }
            }

            if is_screen_member_name(&member.text.contents) {
                // Temporary
                self.drawer.clear(inner_camera, colours::WHITE);
                for member in subgame.members.iter() {
                    let source = sheet_source_rect(member.sprite);
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        inner_camera,
                        drawn_source_rect(member.position, source),
                        &subgame.assets.texture,
                        params,
                    );
                    /*if member.name == "Plum" || member.name == "gdr" {
                        let a_source = sheet_source_rect(member.sprite);
                        let a_obj = CollisionObject {
                            position: member.position.into(),
                            section: a_source,
                            grid: &subgame.assets.image,
                        };
                        for i in 0..200 {
                            for j in 0..200 {
                                let position = Position::new(i, j);
                                if a_obj.is_square_active(position) {
                                    macroquad::prelude::draw_rectangle(
                                        i as f32,
                                        j as f32,
                                        1.0,
                                        1.0,
                                        macroquad::prelude::BLUE,
                                    );
                                }
                            }
                        }
                    }*/

                    if member.text.contents.starts_with("{Image File") {
                        let max_per_page = max_var_per_page(&game.members, "Image File");
                        for i in 1..=max_per_page {
                            let len = padded_len(editor.choices.images.len(), max_per_page);
                            let offset = offset_for_page(editor.page, max_per_page, len);
                            if member.text.contents == format!("{{Image File {}}}", i) {
                                let image_index = i - 1 + offset;
                                if image_index < editor.choices.images.len() {
                                    if editor.choices.images[image_index].texture.is_none() {
                                        let filename = format!(
                                            "{}.png",
                                            editor.choices.images[image_index].name
                                        );
                                        let texture = images_texture(&filename).await.unwrap();

                                        texture.set_filter(FilterMode::Linear);
                                        editor.choices.images[image_index].texture = Some(texture);
                                    }
                                    let texture = editor.choices.images[image_index]
                                        .texture
                                        .as_ref()
                                        .unwrap();

                                    let w = 64;
                                    self.drawer.draw_texture(
                                        inner_camera,
                                        drawn_square(member.position, w),
                                        texture,
                                    );
                                }
                            }
                        }
                    }

                    self.drawer.draw_bitmap_text(
                        inner_camera,
                        member.position,
                        &member.text.contents,
                        member.text.colour,
                        &subgame.assets.font,
                    );
                }

                if editor.inner_copy.is_none()
                    && member.text.contents == EDITABLE_SCREEN_NAME
                    && !macroquad::input::is_key_down(KeyCode::LeftShift)
                {
                    let col = |r, g, b| Colour::from_rgba(r, g, b, 255);
                    let colours = [
                        //col(0, 0, 0),
                        col(126, 37, 83),   // PURPLE
                        col(194, 195, 199), // LIGHT GREY
                        col(0, 228, 54),    // GREEN
                        col(171, 82, 54),   // BROWN
                        //col(255, 163, 0), // ORANGE
                        col(29, 43, 83), // DARK_BLUE
                        col(0, 135, 81), // DARK GREEN
                        col(95, 87, 79), // DaRK GREY
                        //col(255, 241, 232),
                        col(255, 0, 77), // RED
                        //col(255, 236, 39), // YELLOW
                        col(41, 173, 255),  // BLUE
                        col(131, 118, 156), // LAVENDER
                        col(255, 119, 168), // PINK
                        col(255, 204, 170), // LIGHT PEACH
                        col(41, 24, 20),    // BROWNISH BLACK
                        col(17, 29, 53),    // DARKER BLUE
                        col(66, 33, 54),    // DARKER PURPLE
                        col(18, 83, 89),    // BLUE GREEN
                        col(116, 47, 41),   // DARK BROWN
                        col(73, 51, 59),    // DARKER GREY
                        col(162, 136, 121), // MEDIUM GREY
                        col(243, 239, 125), // LIGHT YELLOW
                        col(190, 18, 80),   // DARK RED
                        col(255, 108, 36),  // DARK ORANGE
                        col(168, 231, 46),  // LIME GREEN
                        col(0, 181, 67),    // MEDIUM GREE
                        col(6, 90, 181),    // TRUE BLUE
                        col(117, 70, 101),  // MAUVE
                        col(255, 110, 89),  // DARK PEACH
                        col(255, 157, 129), // PEACH
                    ];
                    for (i, member) in subgame.members.iter().enumerate() {
                        let is_in_move_mode = environment.context["Editor Mode"] == "Move";
                        if is_in_move_mode && i != editor.selected_index {
                            continue;
                        }
                        let size = general_area_size(member, &subgame.assets.font);
                        let rect = pixels::Rect::from_centre(member.position.into(), size);
                        let left = rect.min.x;
                        let top = rect.min.y;
                        let right = rect.max.x;
                        let bottom = rect.max.y;
                        for x in (left - 8..=right).step_by(8) {
                            for y in (top - 8..=bottom).step_by(8) {
                                let sx = if x == left - 8 {
                                    24
                                } else if x == right {
                                    40
                                } else {
                                    32
                                };
                                let sy = if y == top - 8 {
                                    0
                                } else if y == bottom {
                                    16
                                } else {
                                    8
                                };
                                if sx == 32 && sy == 8 {
                                    continue;
                                }
                                let mut colour = colours[i % colours.len()];
                                if i != editor.selected_index {
                                    if is_position_in_general_area(
                                        input.inner.position,
                                        member,
                                        &subgame.assets.font,
                                    ) {
                                        colour.a = 0.4;
                                    } else {
                                        colour.a = 0.2;
                                    }
                                }

                                let source = pixels::Rect::from_top_left(
                                    pixels::Position::new(sx, sy),
                                    pixels::Size::square(8),
                                );
                                let params = DrawParams {
                                    source: Some(source),
                                    colour,
                                    ..Default::default()
                                };
                                // TODO: Draw using base texture fn
                                self.drawer.draw_params_texture(
                                    inner_camera,
                                    pixels::Rect::from_top_left(
                                        pixels::Position::new(x, y),
                                        source.size(),
                                    ),
                                    //drawn_source_rect(Vec2::new(x as f32, y as f32), source),
                                    &self.base_texture.clone(),
                                    params,
                                );

                                if i == editor.selected_index {
                                    let frame_per_sixty = game.frame_number % 60;
                                    let x_offset = (frame_per_sixty / 15) as i32;
                                    let source = pixels::Rect::from_top_left(
                                        pixels::Position::new(sx - 24 + x_offset * 24, sy + 24),
                                        pixels::Size::square(8),
                                    );
                                    let params = DrawParams {
                                        source: Some(source),
                                        colour,
                                        ..Default::default()
                                    };
                                    self.drawer.draw_params_texture(
                                        inner_camera,
                                        pixels::Rect::from_top_left(
                                            pixels::Position::new(x, y),
                                            source.size(),
                                        ),
                                        &self.base_texture.clone(),
                                        params,
                                    )
                                }
                            }
                        }
                    }
                }
            }

            if member.text.contents == CHOOSE_AREA_NAME {
                let min_x = environment.context["MinX"].parse().unwrap_or(0);
                let min_y = environment.context["MinY"].parse().unwrap_or(0);
                let max_x = environment.context["MaxX"].parse().unwrap_or(0);
                let max_y = environment.context["MaxY"].parse().unwrap_or(0);

                let screen_rect = pixels::Rect::aabb(
                    min_x.min(max_x),
                    min_y.min(max_y),
                    min_x.max(max_x),
                    min_y.max(max_y),
                );

                let params = DrawParams {
                    colour: colours::RED,
                    thickness: 2,
                    ..Default::default()
                };

                self.drawer
                    .draw_params_rectangle_lines(inner_camera, screen_rect, params);
            }
            if member.text.contents == CHOOSE_POINT_NAME {
                let x = environment.context["X"].parse().unwrap_or(0);
                let y = environment.context["Y"].parse().unwrap_or(0);

                let offset = 1;
                let screen_rect =
                    pixels::Rect::aabb(x - offset, y - offset, x + offset, y + offset);

                let params = DrawParams {
                    colour: colours::RED,
                    ..Default::default()
                };

                self.drawer
                    .draw_params_rectangle(inner_camera, screen_rect, params);
            }
        }

        if let Transition::FadeOut { .. } = transition {
            for member in &subgame.members {
                let source = sheet_source_rect(member.sprite);

                let params = DrawParams {
                    source: Some(source),
                    ..Default::default()
                };

                // TODO: Temporary
                //params.colour = colours::AMBER;

                self.drawer.draw_params_texture(
                    inner_camera,
                    drawn_source_rect(member.position, source),
                    &subgame.assets.texture,
                    params,
                );
            }
        }

        match transition {
            Transition::FadeIn {
                game: fade_game,
                fade_left,
            }
            | Transition::FadeOut {
                game: fade_game,
                fade_left,
            } => {
                let fade_step = 1.0 / FADE_LEN as f32;
                let alpha = *fade_left as f32 * fade_step;
                let scale = Some((1.0 - alpha) * 3.0 + 1.0);

                for member in &fade_game.members {
                    let source = sheet_source_rect(member.sprite);

                    let mut params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };
                    params.colour = quad_colours::WHITE;
                    params.colour.a = alpha;

                    params.scale = scale;

                    //log::debug!("SCALE: {:?}", params.scale);
                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(member.position, source),
                        &fade_game.assets.texture,
                        params,
                    );
                }
                *fade_left -= 1;
                if *fade_left <= 0 {
                    *transition = Transition::None;
                }
            }
            Transition::None => {}
        }

        // For trailer
        if TEMP_TESTING_INTRO_TEXT && game.frame_number < 240 {
            let params = DrawParams {
                colour: colours::WHITE,
                ..Default::default()
            };
            let intro_text = "Release Date: Late";
            self.drawer.draw_bitmap_text_params(
                Camera::Outer,
                (OUTER_CENTRE - pixels::Position::new(0, 16)).into(),
                intro_text,
                &self.intro_font,
                params,
            );
        }

        // TODO: Make demand for showing/hiding intro text
        if false {
            if transition.is_none() && navigation.queue.index % 2 == 0 {
                // TODO: Editor
                // if crate::INITIAL_GAME_NAME != "MzzZZaker" &&  ...
                if game.frame_number > 210 {
                    let n = game.frame_number - 210;
                    let scale = (4.0 - n as f32 / 5.0).max(1.0);
                    let params = DrawParams {
                        colour: colours::WHITE,
                        scale: Some(scale),
                        ..Default::default()
                    };
                    let intro_text = match &subgame.intro_text {
                        IntroText::Same(s) => s,
                        IntroText::Levels(levels) => &levels[environment.difficulty_level as usize],
                    };

                    self.drawer.draw_bitmap_text_params(
                        Camera::Outer,
                        (OUTER_CENTRE - pixels::Position::new(0, 16)).into(),
                        intro_text,
                        &self.intro_font,
                        params,
                    );
                }
                //  && crate::INITIAL_GAME_NAME != "MzzZZaker"
            } else if subgame.frame_number < 30 {
                let params = DrawParams {
                    colour: colours::WHITE,
                    ..Default::default()
                };
                let intro_text = match &subgame.intro_text {
                    IntroText::Same(s) => s,
                    IntroText::Levels(levels) => &levels[environment.difficulty_level as usize],
                };
                self.drawer.draw_bitmap_text_params(
                    Camera::Outer,
                    (OUTER_CENTRE - pixels::Position::new(0, 16)).into(),
                    intro_text,
                    &self.intro_font,
                    params,
                );
            }
        }

        // TODO:
        if game.music_maker_member().is_some() {
            let note_adjust = music_maker.note_adjust();
            let y_pixel_offset = 43.0;
            let note_height = music_maker.note_height();
            let squashed_sheet_offset = if music_maker.is_extended_keyboard() {
                64
            } else {
                0
            };
            let max_offset_here = music_maker.max_offset();

            for member in &game.members {
                if member.text.contents == MUSIC_MAKER_NAME {
                    let mut index_offset = if music_maker.is_extended_keyboard() {
                        2
                    } else {
                        0
                    };
                    index_offset += if music_maker.is_alternative_signature() {
                        1
                    } else {
                        0
                    };
                    let source = sheet_source_rect(Sprite {
                        index: 1 + music_maker.editing_position.page * 4 + index_offset,
                        size: SpriteSize::OuterBg,
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };
                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(member.position, source),
                        &self.music_texture,
                        params,
                    );

                    let rough_octave_span = rough_octave_span(music_maker);
                    let sprite_offset = sprite_offset(music_maker);

                    for octave in 0..rough_octave_span {
                        let octave_height = octave_height(music_maker);
                        let rough_offset = rough_offset(octave, octave_height);
                        let note_positions = note_positions_draw(music_maker);

                        for (semitone, ((x, y), sprite_index)) in
                            note_positions.into_iter().enumerate()
                        {
                            let y_offset = y + rough_offset;
                            if is_outside_edge(music_maker, octave, semitone, y_offset) {
                                continue;
                            }
                            let sprite_index = sprite_index + sprite_offset;
                            let y_offset = y_offset - 9.0;
                            let mut sprite = Sprite {
                                index: sprite_index,
                                size: SpriteSize::Square(32),
                            };
                            // TODO:::
                            if input.outer.left_button.is_down()
                                && is_position_in_sprite_sheet_image(
                                    input.outer.position,
                                    (member.position + Vec2::new(x, y_offset)).into(),
                                    sprite,
                                    &self.music_image,
                                )
                            {
                                sprite.index += 16;
                            }
                            let source = sheet_source_rect(sprite);
                            let params = DrawParams {
                                source: Some(source),
                                ..Default::default()
                            };

                            self.drawer.draw_params_texture(
                                Camera::Outer,
                                drawn_source_rect(member.position + Vec2::new(x, y_offset), source),
                                &self.music_texture,
                                params,
                            );
                        }
                    }

                    let stoppers = if music_maker.is_extended_keyboard() {
                        [((-140.0, -72.0), 24), ((-140.0, 77.0), 25)]
                    } else {
                        [((-140.0, -74.0), 24), ((-140.0, 79.0), 25)]
                    };

                    for ((x, y), sprite_index) in stoppers {
                        let y = y - 9.0;
                        let source = sheet_source_rect(Sprite {
                            index: sprite_index + sprite_offset,
                            size: SpriteSize::Square(32),
                        });
                        let params = DrawParams {
                            source: Some(source),
                            ..Default::default()
                        };
                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_source_rect(member.position + Vec2::new(x, y), source),
                            &self.music_texture,
                            params,
                        );
                    }
                }
            }

            for note in music_maker
                .notes()
                .iter()
                .filter(|n| {
                    if music_maker.editing_position.page == 0 {
                        n.offset < max_offset_here
                    } else {
                        n.offset + n.length > max_offset_here
                    }
                })
                .map(|n| RelativeNote {
                    offset: if music_maker.editing_position.page == 1 {
                        n.offset as i32 - max_offset_here as i32
                    } else {
                        n.offset as i32
                    },
                    pitch: n.pitch,
                    length: n.length,
                })
            {
                if note.length == 1 {
                    let source = sheet_source_rect(Sprite {
                        index: 192 + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    let x = note.offset as f32 * 16.0 + 64.0 + 8.0;

                    let y = 216.0
                        - (note.pitch - note_adjust) as f32 * note_height as f32
                        - y_pixel_offset;

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );
                } else {
                    let start_x = note.offset;
                    let x = start_x as f32 * 16.0 + 64.0 + 8.0;

                    let y = 216.0
                        - (note.pitch - note_adjust) as f32 * note_height as f32
                        - y_pixel_offset;

                    let source = sheet_source_rect(Sprite {
                        index: 193 + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    if start_x >= 0 {
                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_source_rect(Vec2::new(x, y), source),
                            &self.music_texture,
                            params,
                        );
                    }
                    if start_x + note.length as i32 > max_offset_here as i32 {
                        // TODO: System font
                        self.drawer.draw_bitmap_text(
                            Camera::Outer,
                            Vec2::new(x, y),
                            &format!("{}", note.length),
                            colours::WHITE,
                            &game.assets.font,
                        );
                    } else {
                        let end_x = start_x + note.length as i32 - 1;
                        let x = end_x as f32 * 16.0 + 64.0 + 8.0;

                        let source = sheet_source_rect(Sprite {
                            index: 195 + squashed_sheet_offset,
                            size: SpriteSize::Square(16),
                        });
                        let params = DrawParams {
                            source: Some(source),
                            ..Default::default()
                        };

                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_source_rect(Vec2::new(x, y), source),
                            &self.music_texture,
                            params,
                        );
                    }

                    for i in 1..(note.length - 1) {
                        let current_x = start_x + i as i32;
                        if current_x < 0 || note.offset + i as i32 >= max_offset_here as i32 {
                            continue;
                        }
                        let x = current_x as f32 * 16.0 + 64.0 + 8.0;

                        let source = sheet_source_rect(Sprite {
                            index: 194 + squashed_sheet_offset,
                            size: SpriteSize::Square(16),
                        });
                        let params = DrawParams {
                            source: Some(source),
                            ..Default::default()
                        };

                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_source_rect(Vec2::new(x, y), source),
                            &self.music_texture,
                            params,
                        );
                        if current_x == 0 {
                            self.drawer.draw_bitmap_text(
                                Camera::Outer,
                                Vec2::new(x, y),
                                &format!("{}", note.length),
                                colours::WHITE,
                                &game.assets.font,
                            );
                        }
                    }
                }
            }

            /*if let Some(note) = preview_note.map(|n| MakerNote {
                offset: if music_maker.editing_position.page == 1
                    && n.offset >= max_offset_here
                {
                    n.offset - max_offset_here
                } else {
                    n.offset
                },
                ..n
            }) */
            if music_maker.tracker.has_preview_note {
                let note = MakerNote {
                    offset: if music_maker.editing_position.page == 1
                        && music_maker.intended_note.offset >= max_offset_here
                    {
                        music_maker.intended_note.offset - max_offset_here
                    } else {
                        music_maker.intended_note.offset
                    },
                    ..music_maker.intended_note
                };
                if note.length == 1 {
                    let source = sheet_source_rect(Sprite {
                        index: 192 + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    let x = note.offset as f32 * 16.0 + 64.0 + 8.0;

                    let y = 216.0
                        - (note.pitch - note_adjust) as f32 * note_height as f32
                        - y_pixel_offset;

                    //log::debug!("{}, {}", x, y);

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );
                } else {
                    let start_x = note.offset;
                    let x = start_x as f32 * 16.0 + 64.0 + 8.0;

                    let y = 216.0
                        - (note.pitch - note_adjust) as f32 * note_height as f32
                        - y_pixel_offset;

                    let source = sheet_source_rect(Sprite {
                        index: 193 + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );
                    if start_x + note.length > max_offset_here {
                        // TODO: System font
                        self.drawer.draw_bitmap_text(
                            Camera::Outer,
                            Vec2::new(x, y),
                            &format!("{}", music_maker.intended_note.length),
                            colours::WHITE,
                            &game.assets.font,
                        );
                    }

                    let end_x = start_x + note.length - 1;
                    let x = end_x as f32 * 16.0 + 64.0 + 8.0;

                    let source = sheet_source_rect(Sprite {
                        index: 195 + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );

                    for i in 1..(note.length - 1) {
                        let current_x = start_x + i;
                        let x = current_x as f32 * 16.0 + 64.0 + 8.0;

                        let source = sheet_source_rect(Sprite {
                            index: 194 + squashed_sheet_offset,
                            size: SpriteSize::Square(16),
                        });
                        let params = DrawParams {
                            source: Some(source),
                            ..Default::default()
                        };

                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_source_rect(Vec2::new(x, y), source),
                            &self.music_texture,
                            params,
                        );
                    }
                }
            }

            if environment.context["Music Mode"] == "Add"
                && music_maker.tracker.has_valid_intended_note
            {
                let mut start_x = music_maker.intended_note.offset;
                if music_maker.editing_position.page == 1 {
                    start_x = start_x.max(max_offset_here) - max_offset_here;
                }
                let x = start_x as f32 * 16.0 + 64.0 + 8.0;

                let note_offset = if music_maker.is_extended_keyboard() {
                    0
                } else {
                    note_adjust
                };
                let y = 216.0
                    - (music_maker.intended_note.pitch - note_offset) as f32 * note_height as f32
                    - y_pixel_offset;

                let is_possible = music_maker.is_note_possible(start_x);
                let sprite_offset = if is_possible { 0 } else { 4 };

                let source = sheet_source_rect(Sprite {
                    index: 224 + sprite_offset + squashed_sheet_offset,
                    size: SpriteSize::Square(16),
                });
                let params = DrawParams {
                    source: Some(source),
                    ..Default::default()
                };

                assert_ne!(music_maker.intended_note.length, 0);
                if music_maker.intended_note.length == 1 {
                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );
                } else {
                    let source = sheet_source_rect(Sprite {
                        index: 225 + sprite_offset + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );
                    if start_x + music_maker.intended_note.length > max_offset_here {
                        // TODO: System font
                        self.drawer.draw_bitmap_text(
                            Camera::Outer,
                            Vec2::new(x, y),
                            &format!("{}", music_maker.intended_note.length),
                            colours::WHITE,
                            &game.assets.font,
                        );
                    }

                    let end_x = start_x + music_maker.intended_note.length - 1;
                    let x = end_x as f32 * 16.0 + 64.0 + 8.0;

                    let source = sheet_source_rect(Sprite {
                        index: 227 + sprite_offset + squashed_sheet_offset,
                        size: SpriteSize::Square(16),
                    });
                    let params = DrawParams {
                        source: Some(source),
                        ..Default::default()
                    };

                    self.drawer.draw_params_texture(
                        Camera::Outer,
                        drawn_source_rect(Vec2::new(x, y), source),
                        &self.music_texture,
                        params,
                    );

                    for i in 1..(music_maker.intended_note.length - 1) {
                        let current_x = start_x + i;
                        let x = current_x as f32 * 16.0 + 64.0 + 8.0;

                        let source = sheet_source_rect(Sprite {
                            index: 226 + sprite_offset + squashed_sheet_offset,
                            size: SpriteSize::Square(16),
                        });
                        let params = DrawParams {
                            source: Some(source),
                            ..Default::default()
                        };

                        self.drawer.draw_params_texture(
                            Camera::Outer,
                            drawn_source_rect(Vec2::new(x, y), source),
                            &self.music_texture,
                            params,
                        );
                    }
                }
            } else if environment.context["Music Mode"] == "Remove"
                && music_maker.tracker.has_valid_intended_note
            {
                let mut start_x = music_maker.intended_note.offset;
                if music_maker.editing_position.page == 1 {
                    start_x = start_x.max(max_offset_here) - max_offset_here;
                }
                let x = start_x as f32 * 16.0 + 64.0 + 8.0;

                let note_offset = if music_maker.is_extended_keyboard() {
                    0
                } else {
                    note_adjust
                };
                let y = 216.0
                    - (music_maker.intended_note.pitch - note_offset) as f32 * note_height as f32
                    - y_pixel_offset;

                let source = sheet_source_rect(Sprite {
                    index: 80,
                    size: SpriteSize::Square(32),
                });
                let params = DrawParams {
                    source: Some(source),
                    ..Default::default()
                };

                self.drawer.draw_params_texture(
                    Camera::Outer,
                    drawn_source_rect(Vec2::new(x, y), source),
                    &self.music_texture,
                    params,
                );
            }

            for member in &game.members {
                if member.text.contents == MUSIC_MAKER_NAME {
                    // TODO: Draw lines instead of rectangle? or 2 thick rect?
                    if audio_player.has_midi() {
                        let mut pos = audio_player.display_position();

                        if music_maker.editing_position.page == 1 {
                            pos -= if music_maker.is_alternative_signature() {
                                1.5
                            } else {
                                2.0
                            };
                        }

                        let max_x_offset = if music_maker.is_alternative_signature() {
                            64.0
                        } else {
                            128.0
                        };
                        let mut offset = Vec2::new(-128.0 + pos as f32 * 128.0, 2.0);
                        //let measure_view_count = if is_alternative_signature { 12 } else { 16 };
                        let crotchet_width = 16;
                        offset.x = ((offset.x as i32) / crotchet_width * crotchet_width) as f32;
                        offset.y -= 8.0;

                        if offset.x < -128.0 || offset.x > max_x_offset {
                            continue;
                        }
                        //log::debug!("OFFSET, {} pos, {}", offset, pos);
                        let screen_rect = pixels::Rect::from_centre(
                            (member.position + offset).into(),
                            pixels::Size::new(1, 150),
                        );
                        self.drawer
                            .draw_rectangle(Camera::Outer, screen_rect, colours::NULLPURPLE)
                    }
                }
            }
        }
    }

    pub fn draw_sprite_sheet(
        &mut self,
        position: Vec2,
        mouse: Mouse,
        texture: &Texture2D,
        context_variables: &HashMap<String, String>,
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

        // TODO: Mess
        let mut z = 0.0;
        let mut w = 0.0;
        let inc = (pixel_size.w / 8) as f32;

        for i in 0..n_per_sheet {
            let start_x = (pixel_size.w * i) % effective_page_width;
            let start_y = ((pixel_size.w * i) / effective_page_width) * pixel_size.h;
            let was_new_thingy = if i == 0 {
                false
            } else {
                start_y != ((pixel_size.w * (i - 1)) / effective_page_width) * pixel_size.h
            };
            let dest_size = pixels::Size::new(pixel_size.w / 4, pixel_size.h / 4);

            if was_new_thingy {
                z = 0.0;
                w += inc;
            }
            let x = start_x as f32 / 4.0 + z + position.x - 96.0;
            let y = start_y as f32 / 4.0 + w + position.y - 96.0;
            z += inc;

            let drawn_region = drawn_from_top_left(Vec2::new(x, y).into(), dest_size);

            self.drawer.draw_params_rectangle(
                Camera::Outer,
                drawn_region,
                //Colour::new(0.5, 0.2, 0.3, 0.2),
                //colours::LIGHTGREY,
                DrawParams {
                    // justify: Justify::Centre,
                    colour: Colour::new(0.5, 0.5, 0.5, 0.4),
                    ..Default::default()
                },
            );
        }

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
            let source = pixels::Rect::from_top_left(
                pixels::Position::new(start_x as i32, start_y as i32),
                pixel_size,
            );
            let dest_size = pixels::Size::new(pixel_size.w / 4, pixel_size.h / 4);
            let params = DrawParams {
                source: Some(source),
                //justify: Justify::Centre,
                ..Default::default()
            };

            //let x = start_x as f32 / 3.0 + member.position.x - 80.0;
            //let y = start_y as f32 / 3.0 + member.position.y - 80.0;

            if was_new_thingy {
                z = 0.0;
                w += inc;
            }
            let x = start_x as f32 / 4.0 + z + position.x - 96.0;
            let y = start_y as f32 / 4.0 + w + position.y - 96.0;
            z += inc;

            let drawn_region = drawn_from_top_left(Vec2::new(x, y).into(), dest_size);

            self.drawer
                .draw_params_texture(Camera::Outer, drawn_region, texture, params);

            if *context_variables.get("Sprite Index").unwrap() == i.to_string()
                && context_variables.get("Sprite Type").unwrap() != "Empty"
            {
                self.drawer.draw_params_rectangle(
                    Camera::Outer,
                    drawn_region,
                    DrawParams {
                        colour: Colour::new(0.5, 0.2, 0.3, 0.4),
                        ..Default::default()
                    },
                );
            }

            let mouse_position = mouse.position;
            if mouse_position.x >= x as i32
                && mouse_position.x < x as i32 + pixel_size.w as i32 / 4
                && mouse_position.y >= y as i32
                && mouse_position.y < y as i32 + pixel_size.h as i32 / 4
            {
                self.drawer.draw_params_rectangle(
                    Camera::Outer,
                    drawn_region,
                    //Colour::new(0.5, 0.2, 0.3, 0.2),
                    //colours::LIGHTGREY,
                    DrawParams {
                        // justify: Justify::Centre,
                        colour: Colour::new(0.5, 0.2, 0.3, 0.4),
                        ..Default::default()
                    },
                );
            }
        }
    }
}
