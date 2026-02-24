use rstar::{RTreeObject, AABB, PointDistance};
use crate::graph::topology::{NodeId};

#[derive(Clone, PartialEq)]
pub struct SpatialNode {
    pub id: NodeId,
    pub point: [f32; 2],
}

impl RTreeObject for SpatialNode {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.point)
    }
}

impl PointDistance for SpatialNode {
    fn distance_2(&self, point: &[f32; 2]) -> f32 {
        let dx = self.point[0] - point[0];
        let dy = self.point[1] - point[1];
        dx * dx + dy * dy
    }
}