//! Splines, profiles and the sweep that turns them into a mesh.
//!
//! This is the piece rivers, roads, tunnels and cables share: a **curve** (where the thing goes),
//! a **profile** (what its cross-section looks like) and a **sweep** (the mesh the profile draws
//! along the curve). Everything here is plain data and deterministic math, so the same input
//! gives the same mesh on every machine.
//!
//! The curve is a cubic Bézier chain: every point carries its own two handles. Handles are
//! **absolute** world positions, which is what makes them easy to store, edit and send over the
//! protocol; `Spline::through` builds a smooth curve from bare points when all you have is a path.
//!
//! The rules of the house apply to everything a file, a script or a network message can send:
//! points and profile vertices must be finite, the counts are capped, and nothing is built until
//! the data passes (`CurveError`).

mod error;
mod profile;
mod spline;
mod sweep;

#[cfg(test)]
mod tests;

pub use error::CurveError;
pub use profile::{MAX_PROFILE_POINTS, Profile, ProfileKind};
pub use spline::{MAX_SPLINE_POINTS, Spline, SplineFrame, SplinePoint};
pub use sweep::{MAX_SWEEP_STEPS, SweepSettings, sweep};
