use crate::graph::topology::EdgeId;
use rstar::{AABB, PointDistance, RTree, RTreeObject};

#[derive(Clone)]
pub struct SpatialEdge {
    pub id: EdgeId,
    pub a: [f32; 2],
    pub b: [f32; 2],
}

impl RTreeObject for SpatialEdge {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.a[0].min(self.b[0]), self.a[1].min(self.b[1])],
            [self.a[0].max(self.b[0]), self.a[1].max(self.b[1])],
        )
    }
}

impl PointDistance for SpatialEdge {
    fn distance_2(&self, point: &[f32; 2]) -> f32 {
        // Distance to line segment
        let (x, y) = (point[0], point[1]);
        let (x1, y1) = (self.a[0], self.a[1]);
        let (x2, y2) = (self.b[0], self.b[1]);

        let dx = x2 - x1;
        let dy = y2 - y1;

        let t = ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy);

        let t = t.clamp(0.0, 1.0);

        let proj_x = x1 + t * dx;
        let proj_y = y1 + t * dy;

        let dx = x - proj_x;
        let dy = y - proj_y;

        dx * dx + dy * dy
    }
}
