//! River and road path networks: splines, nodes, edges and connections.
//!
//! Data only. Behavior lives in `petunia_sculpt` and `petunia_graph`. No trait hierarchies.

use petunia_math::{CubicBezierSpline, Vec3};
use serde::{Deserialize, Serialize};

use crate::ids::PersistentId;

/// Type of a path network node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathNodeType {
    Source,
    Junction,
    Confluence,
    Sink,
    Intersection,
    Bridge,
    DeadEnd,
}

/// A node connecting path segments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathNode {
    pub id: u64,
    pub node_type: PathNodeType,
    pub position: Vec3,
    pub edges: Vec<u64>,
}

/// An edge following a spline segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathEdge {
    pub id: u64,
    pub spline: CubicBezierSpline,
    pub width: f32,
    pub depth: f32,
    pub material: String,
    pub is_road: bool,
}

/// A network of connected paths (roads or rivers).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathNetwork {
    pub nodes: Vec<PathNode>,
    pub edges: Vec<PathEdge>,
    pub flow_vectors: Vec<[f32; 3]>,
    pub margin_mask: Vec<f32>,
}

impl PathNetwork {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            flow_vectors: Vec::new(),
            margin_mask: Vec::new(),
        }
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Simple river/lake volume descriptor for gameplay integration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterVolume {
    pub object_id: PersistentId,
    pub flow_speed: f32,
    pub float_force: f32,
    pub swim_force: f32,
    pub has_current: bool,
}

impl Default for WaterVolume {
    fn default() -> Self {
        Self {
            object_id: PersistentId(1),
            flow_speed: 1.0,
            float_force: 9.8,
            swim_force: 4.0,
            has_current: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_math::SplinePoint;

    fn spline() -> CubicBezierSpline {
        let mut spline = CubicBezierSpline::new();
        spline.points.push(SplinePoint {
            position: Vec3::ZERO,
            ..SplinePoint::default()
        });
        spline.points.push(SplinePoint {
            position: Vec3::new(10.0, 0.0, 0.0),
            ..SplinePoint::default()
        });
        spline
    }

    #[test]
    fn network_round_trips_through_json() {
        let mut network = PathNetwork::new();
        network.nodes.push(PathNode {
            id: 1,
            node_type: PathNodeType::Junction,
            position: Vec3::ZERO,
            edges: vec![10],
        });
        network.edges.push(PathEdge {
            id: 10,
            spline: spline(),
            width: 4.0,
            depth: 1.0,
            material: "gravel".to_string(),
            is_road: true,
        });
        network.flow_vectors.push([1.0, 0.0, 0.0]);
        network.margin_mask.push(0.5);
        let text = serde_json::to_string(&network).unwrap_or_default();
        let read_back: PathNetwork = serde_json::from_str(&text).unwrap_or_default();
        assert_eq!(read_back, network);
    }

    #[test]
    fn water_volume_round_trips_and_has_finite_defaults() {
        let volume = WaterVolume::default();
        let text = serde_json::to_string(&volume).unwrap_or_default();
        let read_back: WaterVolume = serde_json::from_str(&text).unwrap_or_default();
        assert_eq!(read_back, volume);
        assert!(volume.flow_speed.is_finite() && volume.float_force.is_finite());
    }
}
