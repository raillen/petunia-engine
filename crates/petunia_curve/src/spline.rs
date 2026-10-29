//! A cubic Bézier chain with absolute handles, sampled by arc length.
//!
//! Handles are world positions, not offsets: that is what a file, a command payload and the
//! inspector show, and it removes the ambiguity of "relative to what". `Spline::through` derives
//! smooth handles for a bare path, the same shape a Catmull-Rom spline gives.

use petunia_math::Vec3;

use crate::error::CurveError;

/// Most points one spline may have. Files, scripts and network messages are hostile input.
pub const MAX_SPLINE_POINTS: usize = 256;
/// Samples per segment used by the arc-length table. More would be slower without being visible.
const SAMPLES_PER_SEGMENT: usize = 16;

/// One point of the curve: where it is, and the two handles that shape the curve around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplinePoint {
    pub position: Vec3,
    /// Handle of the segment that arrives at this point.
    pub handle_in: Vec3,
    /// Handle of the segment that leaves this point.
    pub handle_out: Vec3,
}

impl SplinePoint {
    /// A sharp corner: both handles sit on the point, so the curve turns here.
    pub fn corner(position: Vec3) -> Self {
        Self {
            position,
            handle_in: position,
            handle_out: position,
        }
    }

    /// A smooth point whose handles follow `direction` by `strength` meters on each side.
    pub fn smooth(position: Vec3, direction: Vec3, strength: f32) -> Self {
        let offset = direction.normalized_or_zero() * strength.max(0.0);
        Self {
            position,
            handle_in: position - offset,
            handle_out: position + offset,
        }
    }

    fn is_finite(&self) -> bool {
        self.position.is_finite() && self.handle_in.is_finite() && self.handle_out.is_finite()
    }
}

/// A frame along the curve: where it is, which way it goes, and a stable pair of right/up axes
/// for placing a profile. `normal` is "right", `binormal` is "up" (see `sweep`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplineFrame {
    pub position: Vec3,
    pub tangent: Vec3,
    pub normal: Vec3,
    pub binormal: Vec3,
}

/// A cubic Bézier chain. Two points make one segment; a closed spline joins the last point back
/// to the first.
#[derive(Clone, Debug, PartialEq)]
pub struct Spline {
    points: Vec<SplinePoint>,
    closed: bool,
}

impl Spline {
    /// Builds a spline from points that already have handles. Everything is validated first.
    pub fn new(points: Vec<SplinePoint>, closed: bool) -> Result<Self, CurveError> {
        if points.len() < 2 {
            return Err(CurveError::TooFewPoints(points.len()));
        }
        if points.len() > MAX_SPLINE_POINTS {
            return Err(CurveError::TooManyPoints {
                count: points.len(),
                maximum: MAX_SPLINE_POINTS,
            });
        }
        if points.iter().any(|point| !point.is_finite()) {
            return Err(CurveError::NonFinite("um ponto da spline"));
        }
        Ok(Self { points, closed })
    }

    /// A smooth spline through bare positions (Catmull-Rom handles, expressed in Bézier terms).
    /// This is the path a creator draws by clicking points.
    pub fn through(positions: &[Vec3], closed: bool) -> Result<Self, CurveError> {
        if positions.is_empty() {
            return Err(CurveError::TooFewPoints(0));
        }
        if positions.len() > MAX_SPLINE_POINTS {
            return Err(CurveError::TooManyPoints {
                count: positions.len(),
                maximum: MAX_SPLINE_POINTS,
            });
        }
        if positions.iter().any(|position| !position.is_finite()) {
            return Err(CurveError::NonFinite("um ponto da spline"));
        }
        let count = positions.len();
        let position_at =
            |index: usize| -> Vec3 { positions.get(index % count).copied().unwrap_or(Vec3::ZERO) };
        let mut points = Vec::with_capacity(count);
        for index in 0..count {
            let position = position_at(index);
            // An open curve starts and ends with the point itself as its neighbour.
            let previous = if index == 0 && !closed {
                position
            } else {
                position_at((index + count - 1) % count)
            };
            let next = if index + 1 == count && !closed {
                position
            } else {
                position_at((index + 1) % count)
            };
            // The chord through the neighbours, a sixth of it on each side: the Catmull-Rom shape.
            let handle = (next - previous) * (1.0 / 6.0);
            points.push(SplinePoint {
                position,
                handle_in: position - handle,
                handle_out: position + handle,
            });
        }
        Self::new(points, closed)
    }

    pub fn points(&self) -> &[SplinePoint] {
        &self.points
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Segments of the chain: one fewer than the points, or one per point when closed.
    pub fn segment_count(&self) -> usize {
        if self.closed {
            self.points.len()
        } else {
            self.points.len().saturating_sub(1)
        }
    }

    /// Lowest and highest control point, or `None` for a spline without points.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let first = self.points.first()?.position;
        let mut minimum = first;
        let mut maximum = first;
        for point in &self.points {
            minimum = Vec3::new(
                minimum.x.min(point.position.x),
                minimum.y.min(point.position.y),
                minimum.z.min(point.position.z),
            );
            maximum = Vec3::new(
                maximum.x.max(point.position.x),
                maximum.y.max(point.position.y),
                maximum.z.max(point.position.z),
            );
        }
        Some((minimum, maximum))
    }

    /// The four control points of a segment, walking back to the first point when closed.
    fn segment(&self, index: usize) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
        if index >= self.segment_count() {
            return None;
        }
        let count = self.points.len();
        let start = self.points.get(index)?;
        let end_index = if index + 1 == count { 0 } else { index + 1 };
        let end = self.points.get(end_index)?;
        Some((
            start.position,
            start.handle_out,
            end.handle_in,
            end.position,
        ))
    }

    /// Position at `t` of the whole curve (`0` is the first point, `1` the last), clamped. The
    /// parameter is uniform per segment, not by arc length: use [`Spline::frames`] to place things
    /// at equal distances.
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let segments = self.segment_count();
        if segments == 0 {
            return self
                .points
                .first()
                .map_or(Vec3::ZERO, |point| point.position);
        }
        let scaled = t.clamp(0.0, 1.0) * segments as f32;
        let index = (scaled.floor() as usize).min(segments - 1);
        let local = scaled - index as f32;
        let Some((p0, p1, p2, p3)) = self.segment(index) else {
            return Vec3::ZERO;
        };
        bezier(p0, p1, p2, p3, local)
    }

    /// Direction of travel at `t`. Falls back to the chord when both handles sit on their points
    /// (a corner), so a corner still has a direction.
    pub fn tangent(&self, t: f32) -> Vec3 {
        let segments = self.segment_count();
        if segments == 0 {
            return Vec3::ZERO;
        }
        let scaled = t.clamp(0.0, 1.0) * segments as f32;
        let index = (scaled.floor() as usize).min(segments - 1);
        let local = scaled - index as f32;
        let Some((p0, p1, p2, p3)) = self.segment(index) else {
            return Vec3::ZERO;
        };
        let from_handles = bezier_derivative(p0, p1, p2, p3, local).normalized_or_zero();
        if from_handles != Vec3::ZERO {
            return from_handles;
        }
        (p3 - p0).normalized_or_zero()
    }

    /// Approximate length, from a fixed sampling of every segment. Deterministic and cheap; the
    /// error is far below the size of a terrain cell.
    pub fn length(&self) -> f32 {
        let table = self.arc_table();
        table.last().map_or(0.0, |entry| entry.0)
    }

    /// A table of `(distance from the start, t)` pairs, sampled at a fixed rate per segment.
    fn arc_table(&self) -> Vec<(f32, f32)> {
        let segments = self.segment_count();
        let mut table = Vec::with_capacity(segments * SAMPLES_PER_SEGMENT + 1);
        if segments == 0 {
            return table;
        }
        let mut previous = self.evaluate(0.0);
        let mut distance = 0.0;
        table.push((0.0, 0.0));
        let total = segments * SAMPLES_PER_SEGMENT;
        for step in 1..=total {
            let t = step as f32 / total as f32;
            let position = self.evaluate(t);
            distance += (position - previous).length();
            previous = position;
            table.push((distance, t));
        }
        table
    }

    /// `steps` frames spread by **equal distance** along the curve, with a frame that does not
    /// spin: each one is the previous rotated as little as possible (parallel transport).
    pub fn frames(&self, steps: usize) -> Result<Vec<SplineFrame>, CurveError> {
        if steps == 0 {
            return Err(CurveError::InvalidSteps(steps));
        }
        let table = self.arc_table();
        let Some(&(total, _)) = table.last() else {
            return Err(CurveError::Degenerate("a spline"));
        };
        if total <= 0.0 {
            return Err(CurveError::Degenerate(
                "a spline com todos os pontos no mesmo lugar",
            ));
        }
        let mut frames = Vec::with_capacity(steps);
        let mut cursor = 0usize;
        let mut previous_normal: Option<Vec3> = None;
        for step in 0..steps {
            // With one ring it goes at the start; otherwise the last ring lands on the end.
            let wanted = if steps == 1 {
                0.0
            } else {
                total * step as f32 / (steps - 1) as f32
            };
            while table.get(cursor + 1).is_some_and(|entry| entry.0 < wanted) {
                cursor += 1;
            }
            let before = table.get(cursor).copied().unwrap_or((0.0, 0.0));
            let t = match table.get(cursor + 1).copied() {
                Some(after) => {
                    let span = after.0 - before.0;
                    if span > 0.0 {
                        let mix = ((wanted - before.0) / span).clamp(0.0, 1.0);
                        before.1 + (after.1 - before.1) * mix
                    } else {
                        before.1
                    }
                }
                None => before.1,
            };
            let tangent = self.tangent(t);
            if tangent == Vec3::ZERO {
                return Err(CurveError::Degenerate("a curva neste ponto"));
            }
            let normal = match previous_normal {
                // Keep the previous "right" as perpendicular to the new tangent as possible.
                Some(previous) => {
                    let projected = previous - tangent * previous.dot(tangent);
                    if projected.length_squared() > 1e-8 {
                        projected.normalized_or_zero()
                    } else {
                        reference_normal(tangent)
                    }
                }
                None => reference_normal(tangent),
            };
            let binormal = tangent.cross(normal).normalized_or_zero();
            if normal == Vec3::ZERO || binormal == Vec3::ZERO {
                return Err(CurveError::Degenerate("o quadro da curva"));
            }
            previous_normal = Some(normal);
            frames.push(SplineFrame {
                position: self.evaluate(t),
                tangent,
                normal,
                binormal,
            });
        }
        Ok(frames)
    }
}

/// A "right" axis for a tangent: world up crossed with the direction, or world X when the curve
/// goes straight up.
fn reference_normal(tangent: Vec3) -> Vec3 {
    let up = Vec3::new(0.0, 1.0, 0.0);
    let candidate = up.cross(tangent).normalized_or_zero();
    if candidate.length_squared() > 1e-8 {
        candidate
    } else {
        let fallback = Vec3::new(1.0, 0.0, 0.0).cross(tangent);
        fallback.normalized_or_zero()
    }
}

fn bezier(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let inverse = 1.0 - t;
    let a = inverse * inverse * inverse;
    let b = 3.0 * inverse * inverse * t;
    let c = 3.0 * inverse * t * t;
    let d = t * t * t;
    Vec3::new(
        p0.x * a + p1.x * b + p2.x * c + p3.x * d,
        p0.y * a + p1.y * b + p2.y * c + p3.y * d,
        p0.z * a + p1.z * b + p2.z * c + p3.z * d,
    )
}

fn bezier_derivative(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let inverse = 1.0 - t;
    let a = 3.0 * inverse * inverse;
    let b = 6.0 * inverse * t;
    let c = 3.0 * t * t;
    (p1 - p0) * a + (p2 - p1) * b + (p3 - p2) * c
}
