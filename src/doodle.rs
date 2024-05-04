use super::pixels;
use super::seeded_rng::{RandomRange, SeededRng};
use crate::colours;
use crate::history::{Event, Step, StepDirection};
use crate::inp::Input;
use crate::meta::{self, Environment};
use crate::play::{self, is_position_in_sized_area, Assets};
use crate::rend::{Image, Texture};
use itertools::Itertools;
use macroquad::{
    color::{colors as quad_colours, Color as Colour},
    logging as log,
    math::Vec2,
    texture::FilterMode,
};
use std::collections::{hash_map::DefaultHasher, BTreeSet};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
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
    updates: &mut HashMap<pixels::Position, (Colour, Colour)>,
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
                    let pos = pixels::Position::new(x as i32, y as i32);
                    let before = image.get_pixel(x, y);
                    image.set_pixel(x, y, colour);
                    // TODO: More efficient way, hashmap or ?
                    if let Some(previous) = updates.get_mut(&pos) {
                        previous.1 = colour;
                    } else {
                        updates.insert(pos, (before, colour));
                    }
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

#[derive(Debug, Clone, Copy, EnumString, Default)]
pub enum Shape {
    #[default]
    Line,
    Rectangle,
    Circle,
}

#[derive(Debug, Clone, Copy, EnumString, Default, PartialEq)]
pub enum ShapeStyle {
    Lined {
        thickness: u32,
    },
    #[default]
    Filled,
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
pub struct PreviewShape {
    pub shape: Shape,
    pub style: ShapeStyle,
    pub area: pixels::Rect,
}

#[derive(Debug, Default)]
pub struct Tracker {
    pub paint_index: usize,
    // TODO: Enum over Shape/Fill?
    pub fill: Option<Fill>,
    pub shape_fill: VecDeque<(pixels::Position, Colour, Colour)>,
    pub temp_clear: bool,
    pub temp_save: bool,
    pub pixel_updates: HashMap<pixels::Position, (Colour, Colour)>,
    pub preview_shape: Option<PreviewShape>,
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

    pub fn draw_stuff(
        &mut self,
        environment: &mut Environment,
        input: &Input,
        tool_position: Vec2,
        assets: &mut Assets,
        events_to_apply: &mut Vec<Event>,
    ) {
        let draw_mode: DrawMode = environment.get_typed_var("Draw Mode").unwrap();

        let brush = draw_mode.brush(&mut environment.rng);

        let paint_image = if draw_mode == DrawMode::Erase {
            &self.erase_paint.image
        } else {
            &self.paint_choices[self.tracker.paint_index].image
        };

        // TODO: Outer mouse?
        if input.inner.left_button.is_down()
            && Self::is_mouse_hovering(tool_position, input.outer.position)
        {
            let mut movement: Vec2 = input.inner.drag.into();

            if input.inner.left_button.is_pressed() {
                movement = Vec2::ZERO;
            }

            draw_using_brush(
                &brush,
                &mut assets.image,
                paint_image,
                input.inner.position,
                movement,
                &mut self.tracker.pixel_updates,
            );

            while (movement.x != 0.0 || movement.y != 0.0) && draw_mode != DrawMode::Spray {
                draw_using_brush(
                    &brush,
                    &mut assets.image,
                    paint_image,
                    input.inner.position,
                    movement,
                    &mut self.tracker.pixel_updates,
                );
                if movement.x.abs() < 1.0 {
                    movement.x = 0.0;
                }
                if movement.y.abs() < 1.0 {
                    movement.y = 0.0;
                }

                movement -= movement.normalize_or_zero();
            }

            assets.texture.update(&assets.image);
        }

        let constrained_input_position = pixels::Position::new(
            input
                .inner
                .position
                .x
                .max(0)
                .min(meta::INNER_WIDTH as i32 - 1),
            input
                .inner
                .position
                .y
                .max(0)
                .min(meta::INNER_HEIGHT as i32 - 1),
        );

        match draw_mode {
            DrawMode::Bucket => {
                if input.outer.left_button.is_pressed()
                    && Self::is_mouse_hovering(tool_position, input.outer.position)
                {
                    if self.tracker.fill.is_none() {
                        // TODO: Remove unwrap
                        let fill_order = environment.get_typed_var("Bucket").unwrap();
                        self.tracker.fill = Some(Fill::new(
                            &assets.image,
                            constrained_input_position,
                            self.tracker.paint_index,
                            fill_order,
                        ));
                    } else {
                        self.tracker.fill = None;
                        if !self.tracker.pixel_updates.is_empty() {
                            let updates =
                                std::mem::replace(&mut self.tracker.pixel_updates, HashMap::new());
                            events_to_apply.push(Event::SetPixels {
                                updates: Rc::new(updates),
                                left_to_right: true,
                            });
                        }
                    }
                }
            }
            DrawMode::Shapes => {
                let shape = environment.get_typed_var::<Shape>("Shape").unwrap();

                let shape_style = environment
                    .get_typed_var::<ShapeStyle>("Shape Style")
                    .unwrap();

                // TODO: Tracker var instead of environment::MinX|Y?
                if input.outer.left_button.is_pressed() {
                    self.tracker.preview_shape = Some(PreviewShape {
                        shape,
                        style: shape_style,
                        area: pixels::Rect {
                            min: constrained_input_position,
                            max: constrained_input_position,
                        },
                    });
                }
                // TODO: ?
                else if input.outer.left_button.is_down() {
                    self.tracker.preview_shape = match self.tracker.preview_shape {
                        Some(PreviewShape { shape, style, area }) => Some(PreviewShape {
                            shape,
                            style: style,
                            area: pixels::Rect {
                                min: area.min,
                                max: constrained_input_position,
                            },
                        }),
                        _ => Some(PreviewShape {
                            shape,
                            style: shape_style,
                            area: pixels::Rect {
                                min: constrained_input_position,
                                max: constrained_input_position,
                            },
                        }),
                    };
                }

                if input.outer.left_button.is_released()
                /*&& Self::is_mouse_hovering(tool_position, input.outer.position)*/
                {
                    if self.tracker.shape_fill.is_empty() {
                        let area = self.tracker.preview_shape.as_ref().map(|s| s.area).unwrap();
                        match shape {
                            Shape::Line => {
                                let mut start = area.min;
                                let end = area.max;
                                let dx = (end.x - start.x).abs();
                                let sx = if start.x < end.x { 1 } else { -1 };
                                let dy = (end.y - start.y).abs();
                                let sy = if start.y < end.y { 1 } else { -1 };
                                let mut err: f32 = 0.5 * if dx > dy { dx } else { -dy } as f32;

                                loop {
                                    let before = assets.image.get_pixel(start.x as _, start.y as _);
                                    let colour = paint_image.get_pixel(
                                        start.x as u32 % PAINT_SIZE as u32,
                                        start.y as u32 % PAINT_SIZE as u32,
                                    );
                                    self.tracker.shape_fill.push_back((start, before, colour));
                                    //self.tracker.pixel_updates.insert(start, (before, colour));
                                    /*assets.image.set_pixel(
                                        start.x as u32,
                                        start.y as u32,
                                        colours::RED,
                                    );*/
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
                            Shape::Rectangle => {
                                let mut set_pixel = |x, y| {
                                    let before = assets.image.get_pixel(x as _, y as _);
                                    let colour = paint_image.get_pixel(
                                        x as u32 % PAINT_SIZE as u32,
                                        y as u32 % PAINT_SIZE as u32,
                                    );
                                    self.tracker.shape_fill.push_back((
                                        pixels::Position::new(x, y),
                                        before,
                                        colour,
                                    ));
                                    /*self.tracker
                                    .pixel_updates
                                    .insert(pixels::Position::new(x, y), (before, colour));*/
                                };
                                let start = area.min();
                                let end = area.max();
                                if shape_style == ShapeStyle::Filled {
                                    for y in start.y..=end.y {
                                        for x in start.x..=end.x {
                                            set_pixel(x, y);
                                        }
                                    }
                                } else {
                                    for y in [start.y, end.y] {
                                        for x in start.x..=end.x {
                                            set_pixel(x, y);
                                        }
                                    }
                                    for x in [start.x, end.x] {
                                        for y in start.y..=end.y {
                                            set_pixel(x, y);
                                        }
                                    }
                                }
                            }
                            Shape::Circle => {
                                let centre: Vec2 = area.centre().into();
                                let rx = area.half_width();
                                let ry = area.half_height();
                                if rx == 0.0 || ry == 0.0 {
                                    return;
                                }
                                let mut recorded_pixels = Vec::new();
                                let mut p = (ry * ry) - (rx * rx * ry) + (0.25 * rx * rx);
                                let mut x = 0.0;
                                let mut y = ry;
                                let mut dx = 2.0 * (rx * ry) * x;
                                let mut dy = 2.0 * (rx * rx) * y;
                                let mut set_pixel = |x: f32, y: f32| {
                                    let px = centre.x + x;
                                    let py = centre.y + y;
                                    // TODO: Also check max
                                    if px >= 0.0 && py >= 0.0 {
                                        //assets.image.set_pixel(px as u32, py as u32, colours::RED);
                                        let before = assets.image.get_pixel(px as _, py as _);
                                        let colour = paint_image.get_pixel(
                                            px as u32 % PAINT_SIZE as u32,
                                            py as u32 % PAINT_SIZE as u32,
                                        );
                                        if shape_style != ShapeStyle::Filled {
                                            self.tracker.shape_fill.push_back((
                                                pixels::Position::new(px as _, py as _),
                                                before,
                                                colour,
                                            ));
                                        } else {
                                            recorded_pixels.push((px as i32, py as i32));
                                        }
                                        /*self.tracker.pixel_updates.insert(
                                            pixels::Position::new(px as _, py as _),
                                            (before, colour),
                                        );*/
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

                                p = (x + 0.5) * (x + 0.5) * ry * ry
                                    + (y - 1.0) * (y - 1.0) * rx * rx
                                    - rx * rx * ry * ry;

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

                                if shape_style == ShapeStyle::Filled {
                                    let mut mins: HashMap<i32, i32> = HashMap::new();
                                    let mut maxes: HashMap<i32, i32> = HashMap::new();
                                    for (px, py) in recorded_pixels {
                                        let min_x = mins
                                            .get(&py)
                                            .copied()
                                            .map(|x: i32| x.min(px))
                                            .unwrap_or(px);
                                        mins.insert(py, min_x);
                                        let max_x = maxes
                                            .get(&py)
                                            .copied()
                                            .map(|x: i32| x.max(px))
                                            .unwrap_or(px);
                                        maxes.insert(py, max_x);
                                    }

                                    for hmm in mins.keys().sorted() {
                                        let start = pixels::Position::new(mins[hmm], *hmm);
                                        let end = pixels::Position::new(maxes[hmm], *hmm);

                                        for px in start.x..=end.x {
                                            for py in start.y..=end.y {
                                                let before =
                                                    assets.image.get_pixel(px as _, py as _);
                                                let colour = paint_image.get_pixel(
                                                    px as u32 % PAINT_SIZE as u32,
                                                    py as u32 % PAINT_SIZE as u32,
                                                );
                                                self.tracker.shape_fill.push_back((
                                                    pixels::Position::new(px as _, py as _),
                                                    before,
                                                    colour,
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        self.tracker.preview_shape = None;
                    } else {
                        self.tracker.shape_fill.clear();
                        if !self.tracker.pixel_updates.is_empty() {
                            let updates =
                                std::mem::replace(&mut self.tracker.pixel_updates, HashMap::new());
                            events_to_apply.push(Event::SetPixels {
                                updates: Rc::new(updates),
                                left_to_right: true,
                            });
                        }
                    }
                }

                let max_shape_fill_per_frame = match (shape, shape_style) {
                    (Shape::Line, _) => 2,
                    (Shape::Rectangle, ShapeStyle::Filled) => 128,
                    (Shape::Circle, ShapeStyle::Filled) => 128,
                    (Shape::Rectangle, _) => 4,
                    (Shape::Circle, _) => 4,
                };
                for _ in 0..max_shape_fill_per_frame {
                    if let Some((position, from, to)) = self.tracker.shape_fill.pop_front() {
                        self.tracker.pixel_updates.insert(position, (from, to));
                        assets.image.set_pixel(position.x as _, position.y as _, to);

                        // TODO: Do elsewhere?
                        assets.texture.update(&assets.image);

                        if self.tracker.shape_fill.is_empty() {
                            // TODO: Remove clone
                            events_to_apply.push(Event::SetPixels {
                                updates: Rc::new(self.tracker.pixel_updates.clone()),
                                left_to_right: true,
                            });
                            self.tracker.pixel_updates = HashMap::new();
                        }
                    }
                }
            }
            DrawMode::Erase => {
                // TODO: ?
                for x in 0..meta::OUTER_WIDTH {
                    for y in 0..meta::OUTER_HEIGHT {
                        assets.image.set_pixel(x, y, colours::WHITE);
                    }
                }

                // assets.texture.update(&assets.image);
            }
            _ => {}
        }

        match draw_mode {
            DrawMode::Bucket => {}
            _ => {
                if self.tracker.fill.is_some() && !self.tracker.pixel_updates.is_empty() {
                    // TODO: Remove clone
                    events_to_apply.push(Event::SetPixels {
                        updates: Rc::new(self.tracker.pixel_updates.clone()),
                        left_to_right: true,
                    });
                    self.tracker.pixel_updates = HashMap::new();
                }

                self.tracker.fill = None;
            }
        }

        if self.tracker.temp_clear {
            let mut temp_pixel_updates: HashMap<pixels::Position, (Colour, Colour)> =
                HashMap::new();
            for x in 0..meta::INNER_WIDTH {
                for y in 0..meta::INNER_HEIGHT {
                    let from = assets.image.get_pixel(x, y);
                    let to = colours::WHITE;
                    assets.image.set_pixel(x, y, to);
                    let pos = pixels::Position::new(x as i32, y as i32);
                    //self.tracker.pixel_updates.insert(pos, (from, to));
                    temp_pixel_updates.insert(pos, (from, to));
                }
            }
            events_to_apply.push(Event::SetPixels {
                updates: Rc::new(temp_pixel_updates),
                left_to_right: true,
            });
            assets.texture.update(&assets.image);
        }

        self.fill_in(&mut assets.image);

        if let Some(fill) = &self.tracker.fill {
            assets.texture.update(&assets.image);

            if fill.pixels.is_empty() {
                self.tracker.fill = None;

                if !self.tracker.pixel_updates.is_empty() {
                    // TODO: Remove clone
                    events_to_apply.push(Event::SetPixels {
                        updates: Rc::new(self.tracker.pixel_updates.clone()),
                        left_to_right: true,
                    });
                    self.tracker.pixel_updates = HashMap::new();
                }
            }
        }

        if (self.tracker.temp_clear || input.outer.left_button.is_released())
            && self.tracker.fill.is_none()
            && self.tracker.shape_fill.is_empty()
            && !self.tracker.pixel_updates.is_empty()
        {
            // TODO: Remove clone
            events_to_apply.push(Event::SetPixels {
                updates: Rc::new(self.tracker.pixel_updates.clone()),
                left_to_right: true,
            });
            self.tracker.pixel_updates = HashMap::new();
        }

        self.tracker.temp_clear = false;
    }

    pub fn is_mouse_hovering(tool_position: Vec2, mouse_position: pixels::Position) -> bool {
        const DRAW_TOOL_WIDTH: u32 = meta::INNER_WIDTH;
        const DRAW_TOOL_HEIGHT: u32 = meta::INNER_HEIGHT;
        let rect = pixels::Rect::from_centre(
            tool_position.into(),
            pixels::Size::new(DRAW_TOOL_WIDTH, DRAW_TOOL_HEIGHT),
        );
        rect.contains_point(mouse_position)
    }

    pub fn fill_in(&mut self, image: &mut Image) {
        if let Some(fill) = &mut self.tracker.fill {
            // TODO: Proper lopp?
            let mut i = 0;
            while let Some(pos) = fill.pixels.pop_first() {
                let x = pos.pos.x as u32;
                let y = pos.pos.y as u32;
                let before = image.get_pixel(x, y);
                let colour = self.paint_choices[fill.applied_colour_index]
                    .image
                    .get_pixel(x % PAINT_SIZE as u32, y % PAINT_SIZE as u32);
                self.tracker.pixel_updates.insert(pos.pos, (before, colour));
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
                if i >= PAINT_SPEED {
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
