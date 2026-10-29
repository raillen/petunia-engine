//! Dragging a profile along a curve: the mesh a road, a river bed or a cable is made of.
//!
//! The rules that make the result usable:
//!
//! * the rings are placed by **equal distance**, so a tight curve does not get a clump of rings;
//! * the frame travels by **parallel transport** (see [`Spline::frames`](crate::Spline)), so the
//!   profile does not spin around the curve;
//! * an **open** profile faces up (a road surface, a river bed); a **closed** one faces outward
//!   (a pipe, a cable). Closed profiles are normalized to counter-clockwise in [`Profile::new`],
//!   so the outward side is not up to the author.

use petunia_math::PolygonMesh;

use crate::error::CurveError;
use crate::profile::Profile;
use crate::spline::Spline;

/// Most rings one sweep may place. 512 rings by 64 profile vertices is 32 768 vertices, inside the
/// mesh limits.
pub const MAX_SWEEP_STEPS: usize = 512;

/// How the sweep is sampled and closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SweepSettings {
    /// Rings along the curve, including both ends. Clamped to `1 ..= MAX_SWEEP_STEPS`.
    pub steps: usize,
    /// Close the ends with a face. Only meaningful for a closed profile (a tube); a ribbon has no
    /// inside to cap.
    pub cap_ends: bool,
}

impl Default for SweepSettings {
    fn default() -> Self {
        Self {
            steps: 24,
            cap_ends: true,
        }
    }
}

/// Builds the mesh of `profile` dragged along `spline`.
pub fn sweep(
    spline: &Spline,
    profile: &Profile,
    settings: &SweepSettings,
) -> Result<PolygonMesh, CurveError> {
    let steps = settings.steps;
    if steps == 0 || steps > MAX_SWEEP_STEPS {
        return Err(CurveError::InvalidSteps(steps));
    }
    let frames = spline.frames(steps)?;
    let ring = profile.points().len();
    let mut mesh = PolygonMesh {
        positions: Vec::with_capacity(steps * ring),
        faces: Vec::new(),
    };
    for frame in &frames {
        for point in profile.points() {
            let offset = frame.normal * point.x + frame.binormal * point.y;
            let position = frame.position + offset;
            if !position.is_finite() {
                return Err(CurveError::NonFinite("um vértice da varredura"));
            }
            mesh.positions.push(position);
        }
    }
    let last_ring_vertex = |step: usize, vertex: usize| -> u32 { (step * ring + vertex) as u32 };
    let edges = if profile.closed() { ring } else { ring - 1 };
    for step in 0..steps.saturating_sub(1) {
        for edge in 0..edges {
            let next = (edge + 1) % ring;
            let a0 = last_ring_vertex(step, edge);
            let a1 = last_ring_vertex(step, next);
            let b0 = last_ring_vertex(step + 1, edge);
            let b1 = last_ring_vertex(step + 1, next);
            // Open profiles face up; closed ones face away from the curve.
            mesh.faces.push(if profile.closed() {
                vec![a0, a1, b1, b0]
            } else {
                vec![b0, b1, a1, a0]
            });
        }
    }
    if settings.cap_ends && profile.closed() && steps >= 2 {
        let first: Vec<u32> = (0..ring)
            .rev()
            .map(|vertex| last_ring_vertex(0, vertex))
            .collect();
        let last: Vec<u32> = (0..ring)
            .map(|vertex| last_ring_vertex(steps - 1, vertex))
            .collect();
        mesh.faces.push(first);
        mesh.faces.push(last);
    }
    // The counts are capped above, but the mesh still has to pass the renderer's own rules.
    mesh.validate()
        .map_err(|error| CurveError::MeshInvalid(error.to_string()))?;
    Ok(mesh)
}
