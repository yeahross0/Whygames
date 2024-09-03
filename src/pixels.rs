use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul, Sub};

// Wouldn't have named this min and max, have to keep min the smaller one after updates
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub min: Position,
    pub max: Position,
}

impl Rect {
    pub const fn new(min: Position, max: Position) -> Rect {
        assert!(max.x >= min.x);
        assert!(max.y >= min.y);
        Rect { min, max }
    }

    pub const fn aabb(ax: i32, ay: i32, bx: i32, by: i32) -> Rect {
        Rect::new(Position::new(ax, ay), Position::new(bx, by))
    }

    pub fn xywh(x: f32, y: f32, w: u32, h: u32) -> Rect {
        let half_width = w as f32 / 2.0;
        let half_height = h as f32 / 2.0;
        Rect::from_half_size([x, y], half_width, half_height)
    }

    pub fn tlwh(x: i32, y: i32, w: u32, h: u32) -> Rect {
        let top_left = Position::new(x, y);
        let size = Size::new(w, h);
        Rect::new(top_left, top_left + size)
    }

    pub fn from_top_left(top_left: Position, size: Size) -> Rect {
        Rect::new(top_left, top_left + size)
    }

    pub fn from_centre(centre: [f32; 2], size: Size) -> Rect {
        let half_width = size.w as f32 / 2.0;
        let half_height = size.h as f32 / 2.0;
        Self::from_half_size(centre, half_width, half_height)
    }

    pub fn from_half_size(centre: [f32; 2], half_width: f32, half_height: f32) -> Rect {
        let [x, y] = centre;
        let min = Position::new((x - half_width) as i32, (y - half_height) as i32);
        let max = Position::new((x + half_width) as i32, (y + half_height) as i32);
        Rect::new(min, max)
    }

    pub fn min(self) -> Position {
        Position::new(self.min.x.min(self.max.x), self.min.y.min(self.max.y))
    }

    pub fn max(self) -> Position {
        Position::new(self.min.x.max(self.max.x), self.min.y.max(self.max.y))
    }

    pub fn bottom_left(self) -> Position {
        Position::new(self.min().x, self.max().y)
    }

    pub fn top_right(self) -> Position {
        Position::new(self.max().x, self.min().y)
    }

    /*pub const fn from_aabb(ax: i32, ay: i32, bx: i32, by: i32) -> Rect {
        let w = bx - ax;
        let h = by - ay;
        Rect::new(ax + w / 2, ay + h / 2, w as u32, h as u32)
        //Rect::new(position.x, position.y, size.w, size.h)
    }*/

    pub fn scale(self, scale: f32) -> Rect {
        let centre = self.centre();
        let size = self.size() * scale;
        Rect::from_centre(centre, size)
    }

    // TODO: ?
    pub fn centre(self) -> [f32; 2] {
        [
            self.min.x.min(self.max.x) as f32 + self.half_width(),
            self.min.y.min(self.max.y) as f32 + self.half_height(),
        ]
    }

    pub fn size(self) -> Size {
        Size::new(self.width(), self.height())
    }

    pub fn width(self) -> u32 {
        (self.max.x - self.min.x).abs() as u32
    }

    pub fn height(self) -> u32 {
        (self.max.y - self.min.y).abs() as u32
    }

    pub fn half_width(self) -> f32 {
        self.width() as f32 / 2.0
    }

    pub fn half_height(self) -> f32 {
        self.height() as f32 / 2.0
    }

    pub fn contains_point(self, position: Position) -> bool {
        let left = self.min.x;
        let top = self.min.y;
        let right = self.max.x;
        let bottom = self.max.y;

        let x = position.x;
        let y = position.y;
        x >= left && x < right && y >= top && y < bottom
    }

    pub fn collides(self, other: Rect) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y > other.max.y
            && self.max.y < other.min.y
    }

    pub fn intersecting_rect(self, other: Rect) -> Option<Rect> {
        if self.collides(other) {
            let result = Rect::aabb(
                self.min.x.max(other.min.x),
                self.min.y.max(other.min.y),
                self.max.x.min(other.max.x),
                self.max.y.min(other.max.y),
            );
            Some(result)
        } else {
            None
        }
    }

    // TODO: Has less meaning because rounded
    /*pub fn top_left(self) -> Position {
        Position::new(self.x - self.w as i32 / 2, self.y - self.h as i32 / 2)
    }*/
}

#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    pub y: i32,
    pub x: i32,
}

impl std::fmt::Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Pos({}, {})", self.x, self.y)
    }
}

impl Position {
    pub const fn new(x: i32, y: i32) -> Position {
        Position { x, y }
    }
}

impl Add<Position> for Position {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x.add(rhs.x),
            y: self.y.add(rhs.y),
        }
    }
}

impl AddAssign<Position> for Position {
    //type Output = Self;
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub<Position> for Position {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x.sub(rhs.x),
            y: self.y.sub(rhs.y),
        }
    }
}

impl Add<Size> for Position {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Size) -> Self {
        Self {
            x: self.x.add(rhs.w as i32),
            y: self.y.add(rhs.h as i32),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl Size {
    pub const fn new(w: u32, h: u32) -> Size {
        Size { w, h }
    }

    pub const fn square(size: u32) -> Size {
        Size::new(size, size)
    }

    pub const fn centre(self) -> Position {
        Position {
            x: self.w as i32 / 2,
            y: self.h as i32 / 2,
        }
    }
}

impl Mul<f32> for Size {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self {
        // TODO: Min 0
        Self {
            w: (self.w as f32 * rhs) as u32,
            h: (self.h as f32 * rhs) as u32,
        }
    }
}
