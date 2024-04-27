use super::pixels::{Position, Rect, Size};

fn is_in_range(x: i32, y: i32, width: i32, height: i32) -> bool {
    if x < 0 || y < 0 {
        return false;
    }
    if x >= width || y >= height {
        return false;
    }
    true
}

pub trait Grid {
    // TODO: Make size
    fn size(&self) -> (i32, i32);

    fn get_square_bit(&self, position: Position) -> bool;

    fn is_square_active(&self, position: Position) -> bool {
        self.is_in_range(position) && self.get_square_bit(position)
    }

    fn is_in_range(&self, position: Position) -> bool {
        let (width, height) = self.size();
        is_in_range(position.x, position.y, width, height)
    }
}

const TEST_SIZE: usize = 4;
impl Grid for [[u8; TEST_SIZE]; TEST_SIZE] {
    fn size(&self) -> (i32, i32) {
        (TEST_SIZE as i32, TEST_SIZE as i32)
    }

    fn get_square_bit(&self, position: Position) -> bool {
        self[position.y as usize][position.x as usize] != 0
    }
}

pub fn _is_subsection_square_active<G: Grid>(grid: &G, pos: Position, section: Rect) -> bool {
    if !is_in_range(
        pos.x,
        pos.y,
        section.width() as i32,
        section.height() as i32,
    ) {
        return false;
    }
    let offset = pos + section.min;
    grid.is_square_active(offset)
}

pub fn is_adjusted_subsection_square_active<G: Grid>(
    grid: &G,
    pos: Position,
    adjustment: Position,
    section: Rect,
) -> bool {
    let left_of_obj = adjustment.x - (section.width() / 2) as i32;
    let top_of_obj = adjustment.y - (section.height() / 2) as i32;
    let top_left = Position::new(left_of_obj, top_of_obj);
    let pos = pos - top_left;
    let offset = pos + section.min;
    if offset.x < section.min.x || offset.y < section.min.y {
        return false;
    }
    if offset.x >= section.max.x || offset.y >= section.max.y {
        return false;
    }
    grid.is_square_active(offset)
}

pub struct GridSection<'a, G> {
    pub section: Rect,
    pub grid: &'a G,
}

impl<'a, G: Grid> Grid for GridSection<'a, G> {
    fn size(&self) -> (i32, i32) {
        let Size { w, h } = self.section.size();
        (w as i32, h as i32)
    }

    fn get_square_bit(&self, position: Position) -> bool {
        let pos = position - self.section.min;
        self.grid.get_square_bit(
            pos
        )
    }
}

impl<'a, G: Grid> GridSection<'a, G> {}

pub struct CollisionObject<'a, G> {
    pub position: Position,
    pub section: Rect,
    pub grid: &'a G,
}

impl<'a, G: Grid> CollisionObject<'a, G> {
    pub fn is_square_active(&self, position: Position) -> bool {
        is_adjusted_subsection_square_active(self.grid, position, self.position, self.section)
    }

    pub fn collides_with_rect(&self, rect: Rect) -> bool {
        for x in rect.min.x..rect.max.x {
            for y in rect.min.y..rect.max.y {
                if self.is_square_active(Position::new(x, y)) {
                    return true;
                }
            }
        }
        false
    }

    // TODO: Boptimise
    pub fn collides_with_other(&self, obj: CollisionObject<'a, G>) -> bool {
        let region_to_check = Rect::xywh(
            self.position.x as f32,
            self.position.y as f32,
            self.section.width(),
            self.section.height(),
        );
        for x in region_to_check.min.x..region_to_check.max.x {
            for y in region_to_check.min.y..region_to_check.max.y {
                if obj.is_square_active(Position::new(x, y)) {
                    return true;
                }
            }
        }
        false
    }
}

//fn do_grid_sections_collide<G: Grid>(a: &G, a_section: Rect, b: &G, b_section: Rect) -> bool (
//if let Some(intersection) = a_section.
//)

//fn is_rect_active_in_grid_section<G: Grid>(grid: &G) -> bool {}

#[cfg(test)]
mod tests {
    //use super::Grid;
    use super::*;

    #[rustfmt::skip]
    const GRID: [[u8; TEST_SIZE]; TEST_SIZE] =
        [[1, 0, 0, 0],
         [0, 0, 1, 0],
         [0, 1, 0, 0],
         [0, 0, 0, 1]];

    #[test]
    fn test_grid() {
        assert!(GRID.is_square_active(Position::new(0, 0)));
        assert!(!GRID.is_square_active(Position::new(0, 3)));
    }

    #[test]
    fn test_outside_grid() {
        #[rustfmt::skip]
        
        assert!(!GRID.is_square_active(Position::new(-5, 0)));
        assert!(!GRID.is_square_active(Position::new(17, 14)));
    }

    #[test]
    fn test_subsection() {
        #[rustfmt::skip]
        
        let section = Rect::aabb(1, 1, 3, 3);
        assert!(
            !_is_subsection_square_active(&GRID, Position::new(0, 0), section)
        );
        assert!(
            _is_subsection_square_active(&GRID, Position::new(1, 0), section)
        );
        assert!(
            _is_subsection_square_active(&GRID, Position::new(0, 1), section)
        );
        assert!(
            !_is_subsection_square_active(&GRID, Position::new(1, 1), section)
        );
    }

    #[test]
    fn test_adjusted_subsection() {
        #[rustfmt::skip]
        
        let section = Rect::aabb(1, 1, 3, 3);
        let adjustment = Position::new(2, 2);
        assert!(
            !is_adjusted_subsection_square_active(&GRID, Position::new(1, 1), adjustment, section)
        );
        assert!(
            is_adjusted_subsection_square_active(&GRID, Position::new(2, 1), adjustment, section)
        );
    }
}
