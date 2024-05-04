use std::collections::HashMap;

use super::art::{Sprite, SpriteSize};
use super::colours;
use super::pixels;
use super::play;
use macroquad::camera::{set_camera as set_quad_camera, Camera2D as QuadCamera};
use macroquad::color::{colors as quad_colours, Color as Colour};
use macroquad::math::Rect;
use macroquad::math::Vec2;
use macroquad::shapes::{draw_circle_lines, draw_line, draw_rectangle};
use macroquad::texture::Texture2D;
use macroquad::window::{self as quad_window};

// TODO: Duped, put these somehwere authoritative
const OUTER_WIDTH: u32 = 384;
const OUTER_HEIGHT: u32 = 216;
const OUTER_SIZE: pixels::Size = pixels::Size::new(OUTER_WIDTH, OUTER_HEIGHT);
const INNER_WIDTH: u32 = 256;
const INNER_HEIGHT: u32 = 144;
const INNER_SIZE: pixels::Size = pixels::Size::new(INNER_WIDTH, INNER_HEIGHT);
pub const SPRITESHEET_WIDTH: u32 = 512;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Camera {
    Outer,
    Inner { position: pixels::Position },
    EditedOuter { position: pixels::Position },
}

impl Camera {
    pub fn to_quad_camera(self) -> QuadCamera {
        let screen_width = quad_window::screen_width();
        let screen_height = quad_window::screen_height();
        let ratio = screen_width / screen_height;
        let intended_ratio = 16.0 / 9.0;
        let outer_rect = || Rect::new(0.0, 0.0, OUTER_WIDTH as f32, OUTER_HEIGHT as f32);
        let scale_to_ratio = |camera: &mut QuadCamera| {
            if ratio > intended_ratio {
                camera.zoom.x /= ratio / intended_ratio;
            } else if ratio < intended_ratio {
                camera.zoom.y *= ratio / intended_ratio;
            }
        };
        let inside_camera = |rect: Rect, position: pixels::Position| {
            let mut camera = QuadCamera::from_display_rect(rect);
            {
                let down_scale = 1.5;
                camera.zoom.x /= down_scale;
                camera.zoom.y /= -down_scale;
            }

            scale_to_ratio(&mut camera);

            {
                let x = position.x as f32;
                let y = position.y as f32;
                let w = OUTER_WIDTH as f32;
                let h = OUTER_HEIGHT as f32;
                let x = (x - w / 2.0) / w;
                let y = (y - h / 2.0) / h;
                camera.offset.x = x * 2.0;
                camera.offset.y = y * 2.0;
            }

            camera
        };
        match self {
            Camera::Outer => {
                let mut camera = QuadCamera::from_display_rect(outer_rect());
                camera.zoom.y *= -1.0;
                scale_to_ratio(&mut camera);
                camera
            }
            Camera::Inner { position } => {
                let rect = Rect::new(0.0, 0.0, INNER_WIDTH as f32, INNER_HEIGHT as f32);
                inside_camera(rect, position)
            }
            Camera::EditedOuter { position } => inside_camera(outer_rect(), position),
        }
    }
}
pub struct Drawer {
    pub camera: Option<Camera>,
    pub scale: f32,
}

impl Drawer {
    pub fn clear_window(&mut self) {
        self.camera = None;
        macroquad::camera::set_default_camera();
        macroquad::window::clear_background(quad_colours::BLACK);
    }

    fn set_camera(&mut self, camera: Camera) {
        self.set_camera_with_scale(camera, 1.0);
    }

    fn set_camera_with_scale(&mut self, camera: Camera, scale: f32) {
        if self.camera != Some(camera) || self.scale != scale {
            let mut quad_camera = camera.to_quad_camera();
            quad_camera.zoom.x *= scale;
            quad_camera.zoom.y *= scale;
            set_quad_camera(&quad_camera);
            self.camera = Some(camera);
            self.scale = scale;
        }
    }

    pub fn clear(&mut self, camera: Camera, colour: Colour) {
        self.set_camera(camera);

        let (width, height) = match camera {
            Camera::Outer | Camera::EditedOuter { .. } => (OUTER_WIDTH, OUTER_HEIGHT),
            Camera::Inner { .. } => (INNER_WIDTH, INNER_HEIGHT),
        };

        draw_rectangle(0.0, 0.0, width as f32, height as f32, colour);
    }

    pub fn draw_debug_line(
        &mut self,
        camera: Camera,
        min: pixels::Position,
        max: pixels::Position,
        colour: Colour,
    ) {
        self.set_camera(camera);
        let thickness = 1.0;
        draw_line(
            min.x as _, min.y as _, max.x as _, max.y as _, thickness, colour,
        )
    }

    pub fn draw_line(
        &mut self,
        camera: Camera,
        mut start: pixels::Position,
        end: pixels::Position,
        colour: Colour,
    ) {
        self.set_camera(camera);
        //let thickness = 1.0;
        let dx = (end.x - start.x).abs();
        let sx = if start.x < end.x { 1 } else { -1 };
        let dy = (end.y - start.y).abs();
        let sy = if start.y < end.y { 1 } else { -1 };
        let mut err: f32 = 0.5 * if dx > dy { dx } else { -dy } as f32;

        loop {
            draw_rectangle(start.x as _, start.y as _, 1.0, 1.0, colour);
            if start.x == end.x && start.y == end.y {
                break;
            }
            let e2 = err;
            if e2 > -dx as f32 {
                err -= dy as f32;
                start.x += sx;
            }
            if e2 < dy as f32 {
                err += dx as f32;
                start.y += sy;
            }
        }
    }

    // TODO: Remove filled parameter, and separate function
    pub fn draw_ellipse_lines(
        &mut self,
        camera: Camera,
        area: pixels::Rect,
        colour: Colour,
        is_filled: bool,
    ) {
        self.set_camera(camera);
        let thickness = 1.0;
        let centre: Vec2 = area.centre().into();
        let rx = area.half_width();
        let ry = area.half_height();
        if rx == 0.0 || ry == 0.0 {
            return;
        }
        let mut p = (ry * ry) - (rx * rx * ry) + (0.25 * rx * rx);
        let mut x = 0.0;
        let mut y = ry;
        let mut dx = 2.0 * (rx * ry) * x;
        let mut dy = 2.0 * (rx * rx) * y;
        let mut recorded_pixels = Vec::new();
        let mut set_pixel = |x: f32, y: f32| {
            let px = centre.x + x;
            let py = centre.y + y;
            // TODO: Also check max
            if px >= 0.0 && py >= 0.0 {
                draw_rectangle(px as _, py as _, 1.0, 1.0, colour);
                recorded_pixels.push((px as i32, py as i32));
            }
        };
        while dy >= dx {
            set_pixel(x, y);
            set_pixel(-x, y);
            set_pixel(x, -y);
            set_pixel(-x, -y);

            if p < 0.0 {
                x += 1.0;
                dx = 2.0 * ry * ry * x;
                p += dx + ry * ry;

                dy = 2.0 * rx * rx * y;
            } else {
                x += 1.0;
                y -= 1.0;
                dx = 2.0 * ry * ry * x;
                dy = 2.0 * rx * rx * y;
                p += dx - dy + ry * ry;
            }
        }

        p = (x + 0.5) * (x + 0.5) * ry * ry + (y - 1.0) * (y - 1.0) * rx * rx - rx * rx * ry * ry;

        while y >= 0.0 {
            set_pixel(x, y);
            set_pixel(-x, y);
            set_pixel(x, -y);
            set_pixel(-x, -y);

            if p > 0.0 {
                y -= 1.0;

                dy = 2.0 * (rx * rx) * y;
                p -= dy + (rx * rx);
            } else {
                x += 1.0;
                y -= 1.0;

                dy -= (2.0 * rx * rx);
                dx += (2.0 * ry * ry);
                p += dx - dy + (rx * rx);
            }
        }

        if is_filled {
            let mut mins: HashMap<i32, i32> = HashMap::new();
            let mut maxes: HashMap<i32, i32> = HashMap::new();
            for (px, py) in recorded_pixels {
                let min_x = mins.get(&py).copied().map(|x: i32| x.min(px)).unwrap_or(px);
                mins.insert(py, min_x);
                let max_x = maxes
                    .get(&py)
                    .copied()
                    .map(|x: i32| x.max(px))
                    .unwrap_or(px);
                maxes.insert(py, max_x);
            }

            for hmm in mins.keys() {
                let start = pixels::Position::new(mins[hmm], *hmm);
                let end = pixels::Position::new(maxes[hmm], *hmm);
                self.draw_line(camera, start, end, colour);
            }
        }
    }

    pub fn draw_debug_circle_lines(
        &mut self,
        camera: Camera,
        position: Vec2,
        radius: f32,
        colour: Colour,
    ) {
        self.set_camera(camera);
        let thickness = 1.0;
        draw_circle_lines(position.x as _, position.y as _, radius, thickness, colour)
    }

    // TODO: Think about best way of composing many arguments to function
    pub fn draw_rectangle(&mut self, camera: Camera, screen_rect: pixels::Rect, colour: Colour) {
        self.draw_params_rectangle(camera, screen_rect, DrawParams::colour(colour));
    }

    // TODO: Think about best way of composing many arguments to function
    pub fn draw_params_rectangle(
        &mut self,
        camera: Camera,
        screen_rect: pixels::Rect,
        params: DrawParams,
    ) {
        self.set_camera(camera);

        // TODO: ?
        /*let position = match params.justify {
            Justify::TopLeft => screen_rect.centre(),
            Justify::Centre => screen_rect.min,
        };*/

        let position = screen_rect.min();

        draw_rectangle(
            position.x as f32,
            position.y as f32,
            screen_rect.width() as f32,
            screen_rect.height() as f32,
            params.colour,
        );
    }

    pub fn draw_rectangle_lines(
        &mut self,
        camera: Camera,
        screen_rect: pixels::Rect,
        colour: Colour,
    ) {
        self.set_camera(camera);
        self.draw_params_rectangle_lines(
            camera,
            screen_rect,
            DrawParams {
                colour,
                thickness: 1,
                ..Default::default()
            },
        );
    }

    pub fn draw_params_rectangle_lines(
        &mut self,
        camera: Camera,
        screen_rect: pixels::Rect,
        params: DrawParams,
    ) {
        self.set_camera(camera);

        macroquad::shapes::draw_rectangle_lines(
            screen_rect.min().x as f32,
            screen_rect.min().y as f32,
            screen_rect.width() as f32,
            screen_rect.height() as f32,
            params.thickness as f32,
            params.colour,
        );
    }

    pub fn draw_texture(&mut self, camera: Camera, screen_rect: pixels::Rect, texture: &Texture2D) {
        self.draw_params_texture(camera, screen_rect, texture, DrawParams::default());
    }

    pub fn draw_params_texture(
        &mut self,
        camera: Camera,
        screen_rect: pixels::Rect,
        texture: &Texture2D,
        params: DrawParams,
    ) {
        let scale = params.scale.unwrap_or(1.0);
        self.set_camera_with_scale(camera, scale);

        // TODO: .5 numbers, and odd sizes
        macroquad::texture::draw_texture_ex(
            texture,
            screen_rect.min.x as f32,
            screen_rect.min.y as f32,
            params.colour,
            macroquad::texture::DrawTextureParams {
                source: params.source.map(|s| s.into()),
                //dest_size: params.size.map(|s| Vec2::new(s.w as f32, s.h as f32)),
                dest_size: Some(Vec2::new(
                    screen_rect.width() as f32,
                    screen_rect.height() as f32,
                )),
                ..Default::default()
            },
        );
    }

    pub fn draw_bitmap_text(
        &mut self,
        camera: Camera,
        centre: Vec2,
        text: &str,
        colour: Colour,
        font: &play::BitmapFont,
    ) {
        self.draw_bitmap_text_params(camera, centre, text, font, DrawParams::colour(colour))
    }

    pub fn draw_bitmap_text_params(
        &mut self,
        camera: Camera,
        centre: Vec2,
        text: &str,
        font: &play::BitmapFont,
        params: DrawParams,
    ) {
        //log::debug!("TEXT: {}", text);
        let total_width = font.text_width(text);

        let mut x = centre.x - (total_width as f32) / 2.0 + 1.0;
        let y = centre.y - font.char_height as f32 / 2.0;
        for letter in text.chars() {
            let letter_index = font.index_from_letter(letter);
            let source = font.source_rects[letter_index];
            let params = DrawParams {
                source: Some(source),

                ..params
            };

            let w = font.source_rects[letter_index].width() as f32;

            self.draw_params_texture(
                camera,
                pixels::Rect::from_top_left(
                    pixels::Position::new(x as i32, y as i32),
                    source.size(),
                ),
                //drawn_source_rect(Vec2::new(x, y), source),
                &font.texture,
                params,
            );
            x += w + 1.0;
        }
    }

    pub fn draw_fancy_text(
        &mut self,
        camera: Camera,
        centre: Vec2,
        fancy_text: &[FancyText],
        colour: Colour,
        font: &play::BitmapFont,
        game_properties: (&Texture2D, play::Size),
    ) {
        let (game_texture, game_size) = game_properties;

        let mut fancy_letters = Vec::new();

        for text in fancy_text {
            match text {
                FancyText::Plain { text } => {
                    for ch in text.chars() {
                        fancy_letters.push(FancyLetter::Letter(ch, colour));
                    }
                }
                FancyText::InColour { text, colour } => {
                    for ch in text.chars() {
                        fancy_letters.push(FancyLetter::Letter(ch, *colour));
                    }
                }
                FancyText::Sprite(sprite) => {
                    fancy_letters.push(FancyLetter::Sprite(*sprite));
                }
                FancyText::Area(area) => {
                    fancy_letters.push(FancyLetter::Area(*area));
                }
                FancyText::Point(position) => {
                    fancy_letters.push(FancyLetter::Point(*position));
                }
            }
        }

        /*let text = bufferimps.iter().fold("".to_string(), |mut acc, (ch, _)| {
            acc.push(*ch);
            acc
        });*/

        //let total_width = drawn_text_width(font, &text);
        let total_width = {
            let mut width = 0;
            for t in &fancy_letters {
                match t {
                    FancyLetter::Letter(ch, _) => {
                        width += font.char_width(*ch) + 1;
                    }
                    FancyLetter::Sprite(_) => {
                        // TODO: Make wide sprites wider here?
                        width += font.char_height;
                    }
                    FancyLetter::Area(_) | FancyLetter::Point(_) => {
                        width += font.char_height * 16 / 9;
                    }
                }
            }
            width
        };

        let (game_width, game_height) = match game_size {
            play::Size::Big => (OUTER_WIDTH, OUTER_HEIGHT),
            play::Size::Small => (INNER_WIDTH, INNER_HEIGHT),
        };
        let mut x = centre.x - (total_width as f32) / 2.0 + 1.0;
        let y = centre.y - font.char_height as f32 / 2.0;
        for fancy in fancy_letters {
            let position = Vec2::new(x, y);
            match fancy {
                FancyLetter::Letter(letter, colour) => {
                    let letter_index = font.index_from_letter(letter);
                    let source = font.source_rects[letter_index];
                    let params = DrawParams {
                        colour,
                        source: Some(source),
                        ..Default::default()
                    };

                    let w = font.char_width(letter) as f32;

                    self.draw_params_texture(
                        camera,
                        drawn_from_top_left(position.into(), source.size()),
                        &font.texture,
                        params,
                    );
                    x += w + 1.0;
                }
                FancyLetter::Sprite(sprite) => {
                    let mut w = font.char_height;
                    let h = font.char_height;
                    if sprite.size == SpriteSize::InnerBg || sprite.size == SpriteSize::OuterBg {
                        w = w * 16 / 9;
                    }
                    let params = DrawParams {
                        source: Some(sheet_source_rect(sprite)),
                        ..Default::default()
                    };

                    let size = pixels::Size::new(w, h);
                    let screen_rect = drawn_from_top_left(position.into(), size);
                    //drawn_sized_rect(position, size);

                    if sprite.size != SpriteSize::Empty {
                        self.draw_params_texture(camera, screen_rect, game_texture, params);
                    }
                    self.draw_rectangle_lines(camera, screen_rect, colours::BLACK);

                    x += w as f32 + 1.0;
                }
                FancyLetter::Area(area) => {
                    let w = font.char_height * 16 / 9;
                    let h = font.char_height;

                    let size = pixels::Size::new(w, h);
                    //let mut pos: pixels::Position = position.into();
                    //pos.y += font.char_height as i32 - h as i32;
                    let screen_rect = drawn_from_top_left(position.into(), size);

                    //drawn_sized_rect(position, size);

                    // TODO: Need to know if for INNER_SIZE or OUTER_SIZE
                    let min_x: f32 = area.min.x as f32 / game_width as f32 * w as f32;
                    let min_y: f32 = area.min.y as f32 / game_height as f32 * h as f32;
                    let max_x: f32 = area.max.x as f32 / game_width as f32 * w as f32;
                    let max_y: f32 = area.max.y as f32 / game_height as f32 * h as f32;

                    //self.draw_params_texture(camera, screen_rect, texture, params);
                    let area = pixels::Rect::aabb(
                        (position.x + min_x) as i32,
                        (position.y + min_y) as i32,
                        (position.x + max_x) as i32,
                        (position.y + max_y) as i32,
                    );
                    self.draw_rectangle_lines(camera, screen_rect, colours::BLACK);
                    self.draw_rectangle(camera, screen_rect, colours::WHITE);
                    self.draw_rectangle_lines(camera, area, colours::RED);

                    x += w as f32 + 1.0;
                }
                FancyLetter::Point(point) => {
                    let w = font.char_height * 16 / 9;
                    let h = font.char_height;

                    let size = pixels::Size::new(w, h);
                    //let mut pos: pixels::Position = position.into();
                    //pos.y += font.char_height as i32 - h as i32;
                    let screen_rect = drawn_from_top_left(position.into(), size);
                    //drawn_sized_rect(position, size);

                    // TODO: Need to know if for INNER_SIZE or OUTER_SIZE
                    let min_x: f32 = point.x as f32 / game_width as f32 * w as f32 - 0.5;
                    let min_y: f32 = point.y as f32 / game_height as f32 * h as f32 - 0.5;
                    let max_x: f32 = point.x as f32 / game_width as f32 * w as f32 + 0.5;
                    let max_y: f32 = point.y as f32 / game_height as f32 * h as f32 + 0.5;

                    //self.draw_params_texture(camera, screen_rect, texture, params);
                    let area = pixels::Rect::aabb(
                        (position.x + min_x) as i32,
                        (position.y + min_y) as i32,
                        (position.x + max_x) as i32,
                        (position.y + max_y) as i32,
                    );
                    self.draw_rectangle_lines(camera, screen_rect, colours::BLACK);
                    self.draw_rectangle(camera, screen_rect, colours::WHITE);
                    self.draw_rectangle(camera, area, colours::RED);

                    x += w as f32 + 1.0;
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DrawParams {
    pub colour: Colour,
    pub source: Option<pixels::Rect>,
    pub thickness: u32,
    pub scale: Option<f32>,
}

impl Default for DrawParams {
    fn default() -> Self {
        DrawParams {
            colour: quad_colours::WHITE,
            source: None,
            thickness: 1,
            scale: None,
        }
    }
}

#[allow(dead_code)]
impl DrawParams {
    fn colour(colour: Colour) -> DrawParams {
        DrawParams {
            colour,
            ..Default::default()
        }
    }
}

// TODO: Location?
#[derive(Debug, Clone)]
pub enum FancyText {
    Plain { text: String },
    InColour { text: String, colour: Colour },
    Sprite(Sprite),
    Area(pixels::Rect),
    Point(pixels::Position),
}

pub fn drawn_rect(centre: Vec2, texture: &Texture2D) -> pixels::Rect {
    let half_width = texture.width() / 2.0;
    let half_height = texture.height() / 2.0;
    pixels::Rect::from_half_size(centre.into(), half_width, half_height)
}

pub fn drawn_source_rect(centre: Vec2, source: pixels::Rect) -> pixels::Rect {
    let half_width = source.width() as f32 / 2.0;
    let half_height = source.height() as f32 / 2.0;
    pixels::Rect::from_half_size(centre.into(), half_width, half_height)
}

pub fn drawn_sized_rect(centre: Vec2, size: pixels::Size) -> pixels::Rect {
    pixels::Rect::from_centre(centre.into(), size)
}

pub fn drawn_from_top_left(top_left: pixels::Position, size: pixels::Size) -> pixels::Rect {
    pixels::Rect::from_top_left(top_left, size)
}

pub fn drawn_square(centre: Vec2, size: u32) -> pixels::Rect {
    pixels::Rect::xywh(centre.x, centre.y, size, size)
}

// TODO: Make methods of Sprite or SpriteSize or whatever
pub fn sprite_size_in_pixels(size: SpriteSize) -> pixels::Size {
    match size {
        SpriteSize::Empty => pixels::Size::new(0, 0),
        SpriteSize::Square(n) => pixels::Size::new(n, n),
        SpriteSize::InnerBg => INNER_SIZE,
        SpriteSize::OuterBg => OUTER_SIZE,
    }
}

pub fn page_width_for_sprite(size: SpriteSize) -> u32 {
    // TODO: Handle n != 2^m
    match size {
        SpriteSize::OuterBg => OUTER_WIDTH,
        _ => SPRITESHEET_WIDTH,
    }
}

// TODO: This is top left position
pub fn position_in_sprite_sheet(sprite: Sprite) -> pixels::Position {
    let size = sprite_size_in_pixels(sprite.size);
    let effective_page_width = page_width_for_sprite(sprite.size);

    let x = (size.w * sprite.index) % effective_page_width;
    let y = (size.w * sprite.index) / effective_page_width * size.h;

    pixels::Position::new(x as i32, y as i32)
}

// TODO: This uses top left position?
pub fn sheet_source_rect(sprite: Sprite) -> pixels::Rect {
    let size = sprite_size_in_pixels(sprite.size);

    let position = position_in_sprite_sheet(sprite);

    pixels::Rect::from_top_left(position, size)
}

pub fn fancy_text_width(fancy_text: &[FancyText], font: &play::BitmapFont) -> u32 {
    let mut fancy_letters = Vec::new();

    let colour = Colour::default();

    for text in fancy_text {
        match text {
            FancyText::Plain { text } => {
                for ch in text.chars() {
                    fancy_letters.push(FancyLetter::Letter(ch, colour));
                }
            }
            FancyText::InColour { text, colour } => {
                for ch in text.chars() {
                    fancy_letters.push(FancyLetter::Letter(ch, *colour));
                }
            }
            FancyText::Sprite(sprite) => {
                fancy_letters.push(FancyLetter::Sprite(*sprite));
            }
            FancyText::Area(area) => {
                fancy_letters.push(FancyLetter::Area(*area));
            }
            FancyText::Point(position) => {
                fancy_letters.push(FancyLetter::Point(*position));
            }
        }
    }

    /*let text = bufferimps.iter().fold("".to_string(), |mut acc, (ch, _)| {
        acc.push(*ch);
        acc
    });*/

    //let total_width = drawn_text_width(font, &text);

    {
        let mut width = 0;
        for t in &fancy_letters {
            match t {
                FancyLetter::Letter(ch, _) => {
                    width += font.char_width(*ch) + 1;
                }
                FancyLetter::Sprite(_) => {
                    // TODO: Make wide sprites wider here?
                    width += font.char_height;
                }
                FancyLetter::Area(_) | FancyLetter::Point(_) => {
                    width += font.char_height * 16 / 9;
                }
            }
        }
        width
    }
}

pub enum FancyLetter {
    Letter(char, Colour),
    Sprite(Sprite),
    Area(pixels::Rect),
    Point(pixels::Position),
}
