use crate::pixels;

pub use macroquad::math::Rect;
pub use macroquad::math::Vec2;

impl From<pixels::Rect> for Rect {
    fn from(rect: pixels::Rect) -> Self {
        Rect::new(
            rect.min.x as f32,
            rect.min.y as f32,
            rect.width() as f32,
            rect.height() as f32,
        )
    }
}

impl From<pixels::Position> for Vec2 {
    fn from(position: pixels::Position) -> Self {
        Vec2::new(position.x as f32, position.y as f32)
    }
}

impl From<Vec2> for pixels::Position {
    fn from(pos: Vec2) -> Self {
        pixels::Position::new(pos.x as i32, pos.y as i32)
    }
}
