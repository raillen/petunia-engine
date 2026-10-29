//! Road navigation graph helpers.
//!
//! Build adjacency for connected road networks, detect dead-ends and islands. Data only: the
//! graph is derived from the serializable [`PathNetwork`] in `petunia_world`.

use std::collections::{BTreeMap, HashSet};

use petunia_world::PathNetwork;
use serde::{Deserialize, Serialize};

/// A navigation node for road/path AI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavNode {
    pub id: u64,
    pub position: [f32; 3],
    /// Edge ids that touch this node.
    pub connected: Vec<u64>,
}

/// A road navigation graph, ready for AI or vehicles.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RoadNavigationGraph {
    pub nodes: Vec<NavNode>,
}

impl RoadNavigationGraph {
    pub fn node(&self, id: u64) -> Option<&NavNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

/// Build a navigation graph from a path network. Every node that carries at least one edge takes
/// part; the node type is kept in the source network for display purposes.
pub fn build_road_navigation(network: &PathNetwork) -> RoadNavigationGraph {
    let nodes = network
        .nodes
        .iter()
        .filter(|node| !node.edges.is_empty())
        .map(|node| NavNode {
            id: node.id,
            position: [node.position.x, node.position.y, node.position.z],
            connected: node.edges.clone(),
        })
        .collect();
    RoadNavigationGraph { nodes }
}

/// Find dead-ends (nodes with exactly one edge).
pub fn find_dead_ends(network: &PathNetwork) -> Vec<u64> {
    let mut dead_ends: Vec<u64> = network
        .nodes
        .iter()
        .filter(|node| node.edges.len() == 1)
        .map(|node| node.id)
        .collect();
    dead_ends.sort_unstable();
    dead_ends
}

/// Find disconnected components (islands) in the road network. Components are ordered by their
/// smallest node id, and nodes inside a component are ordered by id.
pub fn find_islands(network: &PathNetwork) -> Vec<Vec<u64>> {
    let mut edge_nodes: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for node in &network.nodes {
        for edge_id in &node.edges {
            edge_nodes.entry(*edge_id).or_default().push(node.id);
        }
    }
    let mut adjacency: BTreeMap<u64, Vec<u64>> = network
        .nodes
        .iter()
        .map(|node| (node.id, Vec::new()))
        .collect();
    for node_ids in edge_nodes.values() {
        for &first in node_ids {
            for &second in node_ids {
                if first == second {
                    continue;
                }
                if let Some(neighbours) = adjacency.get_mut(&first) {
                    neighbours.push(second);
                }
            }
        }
    }
    let mut visited = HashSet::new();
    let mut components = Vec::new();
    for start in network.nodes.iter().map(|node| node.id) {
        if !visited.insert(start) {
            continue;
        }
        let mut component = vec![start];
        let mut stack = vec![start];
        while let Some(current_id) = stack.pop() {
            let Some(neighbours) = adjacency.get(&current_id) else {
                continue;
            };
            for &neighbour in neighbours {
                if visited.insert(neighbour) {
                    component.push(neighbour);
                    stack.push(neighbour);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components.sort_by_key(|component| component.first().copied().unwrap_or(u64::MAX));
    components
}

/// Check a road network for loose ends, islands and edges without two endpoints.
pub fn validate_road_network(network: &PathNetwork) -> (bool, Vec<String>) {
    let mut issues = Vec::new();
    let dead_ends = find_dead_ends(network);
    if !dead_ends.is_empty() {
        issues.push(format!("dead-ends found: {}", dead_ends.len()));
    }
    let islands = find_islands(network);
    if islands.len() > 1 {
        issues.push(format!("disconnected islands: {}", islands.len()));
    }
    let mut edge_usage: BTreeMap<u64, usize> =
        network.edges.iter().map(|edge| (edge.id, 0)).collect();
    for node in &network.nodes {
        for edge_id in &node.edges {
            if let Some(usage) = edge_usage.get_mut(edge_id) {
                *usage += 1;
            }
        }
    }
    let dangling_edges = edge_usage.values().filter(|usage| **usage < 2).count();
    if dangling_edges > 0 {
        issues.push(format!(
            "edges with fewer than two endpoints: {dangling_edges}"
        ));
    }
    (issues.is_empty(), issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use petunia_world::{PathNode, PathNodeType};

    fn node(id: u64, edges: Vec<u64>) -> PathNode {
        PathNode {
            id,
            node_type: PathNodeType::Intersection,
            position: petunia_math::Vec3::new(id as f32, 0.0, 0.0),
            edges,
        }
    }

    #[test]
    fn navigation_graph_skips_nodes_without_edges() {
        let mut network = PathNetwork::new();
        network.nodes.push(node(1, vec![10]));
        network.nodes.push(node(2, vec![]));
        network.nodes.push(node(3, vec![10, 11]));
        let graph = build_road_navigation(&network);
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.node(1).is_some());
        assert!(graph.node(2).is_none());
    }

    #[test]
    fn islands_are_grouped_by_shared_edges() {
        let mut network = PathNetwork::new();
        network.nodes.push(node(1, vec![10]));
        network.nodes.push(node(2, vec![10, 11]));
        network.nodes.push(node(3, vec![11]));
        network.nodes.push(node(4, vec![20]));
        let islands = find_islands(&network);
        assert_eq!(islands.len(), 2);
        assert_eq!(islands[0], vec![1, 2, 3]);
        assert_eq!(islands[1], vec![4]);
    }

    #[test]
    fn validation_reports_dangling_edges() {
        let mut network = PathNetwork::new();
        network.nodes.push(node(1, vec![10]));
        network.edges.push(petunia_world::PathEdge {
            id: 10,
            spline: petunia_math::CubicBezierSpline::new(),
            width: 3.0,
            depth: 0.0,
            material: "gravel".to_string(),
            is_road: true,
        });
        let (valid, issues) = validate_road_network(&network);
        assert!(!valid);
        assert!(issues.iter().any(|issue| issue.contains("dead-ends")));
        assert!(issues.iter().any(|issue| issue.contains("endpoints")));
    }

    #[test]
    fn dead_ends_are_sorted_and_unique() {
        let mut network = PathNetwork::new();
        network.nodes.push(node(5, vec![10]));
        network.nodes.push(node(3, vec![11]));
        assert_eq!(find_dead_ends(&network), vec![3, 5]);
    }
}
