use super::pixels;
use super::seeded_rng::{RandomRange, SeededRng};
use crate::colours;
use crate::rend::{Image, Texture};
use macroquad::{
    color::{colors as quad_colours, Color as Colour},
    math::Vec2,
    texture::FilterMode,
};
use std::collections::{hash_map::DefaultHasher, BTreeSet};
use std::hash::{Hash, Hasher};
use strum_macros::EnumString;

// TODO: Draw to RenderTarget not to Image
// TODO: Less wide API

pub const PAINT_SIZE: u16 = 16;
const PAINT_SPEED: i32 = 256;

#[derive(Clone, Debug, PartialEq, EnumString, Default)]
pub enum DrawMode {
    #[default]
    Dot,
    Square,
    Circle,
    Bucket,
    Shapes,
    Select,
    Spray,
    Erase,
}

impl DrawMode {
    pub fn brush(&self, rng: &mut SeededRng) -> Vec<Vec<i32>> {
        let mut r = || rng.number_in_range(-12, 2).max(0).min(1);
        match self {
            DrawMode::Dot => vec![vec![1]],
            DrawMode::Square => vec![vec![1, 1, 1], vec![1, 1, 1], vec![1, 1, 1]],
            DrawMode::Circle => vec![
                vec![0, 1, 1, 0],
                vec![1, 1, 1, 1],
                vec![1, 1, 1, 1],
                vec![0, 1, 1, 0],
            ],
            DrawMode::Spray => vec![
                vec![0, 0, 0, 0, r(), r(), r(), r(), 0, 0, 0, 0],
                vec![0, 0, r(), r(), r(), r(), r(), r(), r(), r(), 0, 0],
                vec![0, r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), 0],
                vec![0, r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), 0],
                vec![r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r()],
                vec![r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r()],
                vec![r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r()],
                vec![r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), r()],
                vec![0, r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), 0],
                vec![0, r(), r(), r(), r(), r(), r(), r(), r(), r(), r(), 0],
                vec![0, 0, r(), r(), r(), r(), r(), r(), r(), r(), 0, 0],
                vec![0, 0, 0, 0, r(), r(), r(), r(), 0, 0, 0, 0],
            ],
            // TODO: Temp
            //DrawMode::Erase => vec![vec![1, 1, 1], vec![1, 1, 1], vec![1, 1, 1]],
            _ => vec![vec![0]],
        }
    }
}

pub fn draw_using_brush(
    brush: &[Vec<i32>],
    image: &mut Image,
    paint_image: &Image,
    mouse_position: pixels::Position,
    movement: Vec2,
) {
    for row in 0..brush.len() {
        for column in 0..brush[row].len() {
            if brush[row][column] != 0 {
                let x_offset = (column as isize - brush[row].len() as isize / 2) as f32;
                let y_offset = (row as isize - brush.len() as isize / 2) as f32;
                let x = (mouse_position.x as f32 - movement.x + x_offset) as u32;
                let y = (mouse_position.y as f32 - movement.y + y_offset) as u32;
                let colour = paint_image.get_pixel(x % PAINT_SIZE as u32, y % PAINT_SIZE as u32);
                if x < image.width as u32 && y < image.height as u32 {
                    image.set_pixel(x, y, colour);
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Fill {
    pub order: FillOrder,
    pub original_colour: Colour,
    pub applied_colour_index: usize,
    pub pixels: BTreeSet<DistancedPosition>,
    pub visited_pixels: BTreeSet<pixels::Position>,
    pub start: pixels::Position,
}

impl Fill {
    pub fn new(
        image: &Image,
        mouse_position: pixels::Position,
        paint_index: usize,
        fill_order: FillOrder,
    ) -> Fill {
        let mut fill_queue = BTreeSet::new();
        let pos = mouse_position;
        let original_colour = image.get_pixel(pos.x as u32, pos.y as u32);
        if pos.x >= 0 && pos.y >= 0 && pos.x < image.width as i32 && pos.y < image.height as i32 {
            let dpos = DistancedPosition { dist: 0, pos };
            fill_queue.insert(dpos);
        }

        let mut visited_pixels = BTreeSet::new();
        visited_pixels.insert(pos);

        Fill {
            order: fill_order,
            original_colour,
            applied_colour_index: paint_index,
            pixels: fill_queue,
            visited_pixels,
            start: pos,
        }
    }
}

#[derive(Debug, EnumString)]
pub enum FillOrder {
    Lined,
    Circle,
    Diamond,
    Random,
    Sprawl,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct DistancedPosition {
    pub dist: u64,
    pub pos: pixels::Position,
}

#[derive(Debug)]
pub struct EditableImage {
    pub image: Image,
    pub texture: Texture,
    //image_string: ImageString,
}

impl EditableImage {
    pub fn from_image(image: Image) -> EditableImage {
        let filtered_texture_from_image = |image: &Image| {
            let texture = Texture::from_image(image);
            texture.set_filter(FilterMode::Nearest);
            texture
        };
        EditableImage {
            texture: filtered_texture_from_image(&image),
            image,
        }
    }
}

#[derive(Debug, Default)]
pub struct Tracker {
    pub paint_index: usize,
    pub fill: Option<Fill>,
    pub temp_clear: bool,
    pub temp_save: bool,
}

#[derive(Debug)]
pub struct DrawTool {
    pub paint_choices: Vec<EditableImage>,
    pub erase_paint: EditableImage,
    pub tracker: Tracker,
}

impl DrawTool {
    pub fn init() -> DrawTool {
        let image = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, colours::WHITE);

        DrawTool {
            paint_choices: make_paint_choices(),
            erase_paint: EditableImage::from_image(image),
            tracker: Tracker::default(),
        }
    }

    pub fn fill_in(&mut self, image: &mut Image) {
        if let Some(fill) = &mut self.tracker.fill {
            // TODO: Proper lopp?
            let mut i = 0;
            while let Some(pos) = fill.pixels.pop_first() {
                let x = pos.pos.x as u32;
                let y = pos.pos.y as u32;
                let colour = self.paint_choices[fill.applied_colour_index]
                    .image
                    .get_pixel(x % PAINT_SIZE as u32, y % PAINT_SIZE as u32);
                image.set_pixel(x, y, colour);

                for (x, y) in [(-1, 0), (0, -1), (1, 0), (0, 1)] {
                    let pos = pos.pos + pixels::Position::new(x, y);
                    if pos.x >= 0
                        && pos.y >= 0
                        && pos.x < image.width as i32
                        && pos.y < image.height as i32
                        && image.get_pixel(pos.x as u32, pos.y as u32) == fill.original_colour
                    {
                        let dist: u64 = match fill.order {
                            FillOrder::Lined => (pos.y - fill.start.y).unsigned_abs() as u64,
                            FillOrder::Diamond => {
                                (pos.x - fill.start.x).unsigned_abs() as u64
                                    + (pos.y - fill.start.y).unsigned_abs() as u64
                            }
                            FillOrder::Circle => {
                                (pos.x - fill.start.x).pow(2) as u64
                                    + (pos.y - fill.start.y).pow(2) as u64
                            }
                            FillOrder::Random => {
                                let mut hasher = DefaultHasher::new();
                                pos.hash(&mut hasher);
                                hasher.finish()
                            }
                            FillOrder::Sprawl => fill.pixels.len() as u64,
                        };
                        let dpos = DistancedPosition { dist, pos };
                        if !fill.visited_pixels.contains(&pos) {
                            fill.pixels.insert(dpos);
                        }
                        fill.visited_pixels.insert(pos);
                    }
                }

                i += 1;
                if i > PAINT_SPEED {
                    break;
                }
            }
        }
    }
}

pub fn make_paint_choices() -> Vec<EditableImage> {
    let mut paint_choices = Vec::new();

    let filtered_texture_from_image = |image: &Image| {
        let texture = Texture::from_image(image);
        texture.set_filter(FilterMode::Nearest);
        texture
    };

    let paint_choice = |colour: Colour| {
        let image = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, colour);
        EditableImage {
            texture: filtered_texture_from_image(&image),
            image,
        }
    };

    let striped = |a: Colour, b: Colour| {
        let mut image = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, b);
        for x in (0..PAINT_SIZE).step_by(2) {
            for y in 0..PAINT_SIZE {
                image.set_pixel(x as u32, y as u32, a);
            }
        }
        EditableImage {
            texture: filtered_texture_from_image(&image),
            image,
        }
    };

    let dotted = |a: Colour, b: Colour| {
        let mut image = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, b);
        for x in (0..PAINT_SIZE).step_by(2) {
            for y in (0..PAINT_SIZE).step_by(2) {
                image.set_pixel(x as u32, y as u32, a);
            }
        }
        EditableImage {
            texture: filtered_texture_from_image(&image),
            image,
        }
    };

    let dithered = |a: Colour, b: Colour| {
        let mut image = Image::gen_image_color(PAINT_SIZE, PAINT_SIZE, b);
        for x in 0..PAINT_SIZE {
            for y in 0..PAINT_SIZE {
                if x % 2 == y % 2 {
                    image.set_pixel(x as u32, y as u32, a);
                }
            }
        }
        EditableImage {
            texture: filtered_texture_from_image(&image),
            image,
        }
    };

    let white = colours::WHITE;

    paint_choices.push(paint_choice(colours::BLACK));
    paint_choices.push(paint_choice(colours::LIGHTPINK));
    paint_choices.push(paint_choice(colours::AMBER));
    paint_choices.push(paint_choice(colours::DARKBROWN));
    paint_choices.push(paint_choice(colours::RED));
    paint_choices.push(paint_choice(colours::NULLPURPLE));
    paint_choices.push(paint_choice(colours::SKYBLUE));
    paint_choices.push(paint_choice(colours::BLUE));
    paint_choices.push(paint_choice(colours::GREEN));
    paint_choices.push(paint_choice(colours::DULLGREEN));
    paint_choices.push(paint_choice(colours::DULLWOOD));
    paint_choices.push(paint_choice(colours::NULLYELLOW2));
    paint_choices.push(paint_choice(colours::DARKGREY));
    paint_choices.push(paint_choice(colours::GREY));
    paint_choices.push(paint_choice(colours::LIGHTGREY));
    paint_choices.push(paint_choice(white));

    paint_choices.push(striped(colours::BLACK, colours::DARKGREY));
    paint_choices.push(striped(colours::LIGHTPINK, colours::NULLPURPLE));
    paint_choices.push(striped(colours::AMBER, colours::NULLYELLOW2));
    paint_choices.push(striped(colours::DARKBROWN, colours::DULLWOOD));
    paint_choices.push(striped(colours::RED, colours::AMBER));
    paint_choices.push(striped(colours::NULLPURPLE, white));
    paint_choices.push(striped(colours::SKYBLUE, colours::BLUE));
    paint_choices.push(striped(colours::BLUE, colours::BLACK));
    paint_choices.push(striped(colours::GREEN, colours::DULLGREEN));
    paint_choices.push(striped(colours::DULLGREEN, colours::DULLWOOD));
    paint_choices.push(striped(colours::DULLWOOD, colours::NULLYELLOW2));
    paint_choices.push(striped(colours::NULLYELLOW2, white));
    paint_choices.push(striped(colours::DARKGREY, colours::GREY));
    paint_choices.push(striped(colours::GREY, colours::LIGHTGREY));
    paint_choices.push(striped(colours::LIGHTGREY, white));
    paint_choices.push(striped(white, colours::BLACK));

    paint_choices.push(dotted(colours::BLACK, colours::DARKGREY));
    paint_choices.push(dotted(colours::LIGHTPINK, colours::NULLPURPLE));
    paint_choices.push(dotted(colours::AMBER, colours::NULLYELLOW2));
    paint_choices.push(dotted(colours::DARKBROWN, colours::DULLWOOD));
    paint_choices.push(dotted(colours::RED, colours::AMBER));
    paint_choices.push(dotted(colours::NULLPURPLE, white));
    paint_choices.push(dotted(colours::SKYBLUE, colours::BLUE));
    paint_choices.push(dotted(colours::BLUE, colours::BLACK));
    paint_choices.push(dotted(colours::GREEN, colours::DULLGREEN));
    paint_choices.push(dotted(colours::DULLGREEN, colours::DULLWOOD));
    paint_choices.push(dotted(colours::DULLWOOD, colours::NULLYELLOW2));
    paint_choices.push(dotted(colours::NULLYELLOW2, white));
    paint_choices.push(dotted(colours::DARKGREY, colours::GREY));
    paint_choices.push(dotted(colours::GREY, colours::LIGHTGREY));
    paint_choices.push(dotted(colours::LIGHTGREY, white));
    paint_choices.push(dotted(white, colours::BLACK));

    paint_choices.push(dithered(colours::BLACK, colours::DARKGREY));
    paint_choices.push(dithered(colours::LIGHTPINK, colours::NULLPURPLE));
    paint_choices.push(dithered(colours::AMBER, colours::NULLYELLOW2));
    paint_choices.push(dithered(colours::DARKBROWN, colours::DULLWOOD));
    paint_choices.push(dithered(colours::RED, colours::AMBER));
    paint_choices.push(dithered(colours::NULLPURPLE, white));
    paint_choices.push(dithered(colours::SKYBLUE, colours::BLUE));
    paint_choices.push(dithered(colours::BLUE, colours::BLACK));
    paint_choices.push(dithered(colours::GREEN, colours::DULLGREEN));
    paint_choices.push(dithered(colours::DULLGREEN, colours::DULLWOOD));
    paint_choices.push(dithered(colours::DULLWOOD, colours::NULLYELLOW2));
    paint_choices.push(dithered(colours::NULLYELLOW2, white));
    paint_choices.push(dithered(colours::DARKGREY, colours::GREY));
    paint_choices.push(dithered(colours::GREY, colours::LIGHTGREY));
    paint_choices.push(dithered(colours::LIGHTGREY, white));
    paint_choices.push(dithered(white, colours::BLACK));

    use macroquad::color_u8;
    use macroquad::prelude::Color;

    paint_choices.push(paint_choice(quad_colours::BLACK));
    paint_choices.push(paint_choice(color_u8!(255, 222, 156, 255)));
    paint_choices.push(paint_choice(color_u8!(255, 173, 49, 255)));
    paint_choices.push(paint_choice(color_u8!(198, 74, 0, 255)));
    paint_choices.push(paint_choice(color_u8!(255, 0, 0, 255)));
    paint_choices.push(paint_choice(color_u8!(206, 107, 239, 255)));
    paint_choices.push(paint_choice(color_u8!(16, 198, 206, 255)));
    paint_choices.push(paint_choice(color_u8!(41, 107, 198, 255)));
    paint_choices.push(paint_choice(color_u8!(8, 148, 82, 255)));
    paint_choices.push(paint_choice(color_u8!(115, 214, 57, 255)));
    paint_choices.push(paint_choice(color_u8!(255, 255, 90, 255)));
    paint_choices.push(paint_choice(color_u8!(123, 123, 123, 255)));
    paint_choices.push(paint_choice(color_u8!(198, 198, 198, 255)));
    paint_choices.push(paint_choice(quad_colours::WHITE));
    paint_choices.push(paint_choice(color_u8!(74, 156, 173, 255)));
    paint_choices.push(paint_choice(colours::DULLPINK));

    paint_choices
}
