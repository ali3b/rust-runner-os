/// Axis-aligned bounding box in logical pixels (y grows downward).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Aabb {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// True when the boxes overlap by a positive area. Touching edges do not count.
    pub fn intersects(&self, other: &Aabb) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }

    /// Shrinks the box by `dx` on the left and right and `dy` on the top and bottom.
    pub fn inset(&self, dx: f32, dy: f32) -> Aabb {
        Aabb::new(self.x + dx, self.y + dy, self.w - 2.0 * dx, self.h - 2.0 * dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_boxes_intersect() {
        let a = Aabb::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(&Aabb::new(5.0, 5.0, 10.0, 10.0)));
        assert!(a.intersects(&Aabb::new(2.0, 2.0, 2.0, 2.0)), "contained box");
    }

    #[test]
    fn touching_or_separate_boxes_do_not_intersect() {
        let a = Aabb::new(0.0, 0.0, 10.0, 10.0);
        assert!(!a.intersects(&Aabb::new(10.0, 0.0, 5.0, 5.0)), "touching right edge");
        assert!(!a.intersects(&Aabb::new(0.0, 10.0, 5.0, 5.0)), "touching bottom edge");
        assert!(!a.intersects(&Aabb::new(20.0, 20.0, 5.0, 5.0)));
        assert!(!a.intersects(&Aabb::new(-6.0, 0.0, 5.0, 5.0)));
    }

    #[test]
    fn inset_shrinks_symmetrically() {
        let b = Aabb::new(10.0, 20.0, 30.0, 40.0).inset(2.0, 3.0);
        assert_eq!(b, Aabb::new(12.0, 23.0, 26.0, 34.0));
    }
}
