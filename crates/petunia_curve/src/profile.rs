//! The cross-section that a sweep drags along a curve.
//!
//! A profile is a flat polyline in meters: `x` goes to the **right** of the direction of travel,
//! `y` goes **up**. Open profiles make ribbons and channels (a road, a river bed); closed ones
//! make tubes (a cable, a pipe, a tunnel lining).
//!
//! The three shapes here are the ones the tools start from; anything else is a list of vertices.

use petunia_math::Vec2;

use crate::error::CurveError;

/// Most vertices one profile may have.
pub const MAX_PROFILE_POINTS: usize = 64;

/// The shapes the editor and the commands offer by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileKind {
    /// A flat strip of `width` meters, `thickness` thick: the surface of a road.
    Ribbon,
    /// A channel `width` wide and `depth` deep, with sloped banks: a river bed.
    Trough,
    /// A closed ring of `sides` vertices, `radius` from the center: a pipe or a cable.
    Tube,
}

impl ProfileKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ribbon => "ribbon",
            Self::Trough => "trough",
            Self::Tube => "tube",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "ribbon" | "faixa" => Some(Self::Ribbon),
            "trough" | "calha" => Some(Self::Trough),
            "tube" | "tubo" => Some(Self::Tube),
            _ => None,
        }
    }

    /// Builds the shape. `width` is the full width (or the diameter of a tube), `depth` how deep
    /// a trough goes, `thickness` how thick a ribbon is and `sides` how many vertices a tube has.
    /// Values that would make a zero-width shape are rejected.
    pub fn build(
        self,
        width: f32,
        depth: f32,
        thickness: f32,
        sides: usize,
    ) -> Result<Profile, CurveError> {
        match self {
            Self::Ribbon => Profile::ribbon(width, thickness),
            Self::Trough => Profile::trough(width, depth),
            Self::Tube => Profile::tube(width * 0.5, sides),
        }
    }
}

/// A cross-section: vertices in meters, open or closed.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    points: Vec<Vec2>,
    closed: bool,
}

impl Profile {
    pub fn new(points: Vec<Vec2>, closed: bool) -> Result<Self, CurveError> {
        if points.len() < 2 {
            return Err(CurveError::TooFewPoints(points.len()));
        }
        if points.len() > MAX_PROFILE_POINTS {
            return Err(CurveError::TooManyPoints {
                count: points.len(),
                maximum: MAX_PROFILE_POINTS,
            });
        }
        if points.iter().any(|point| !point.is_finite()) {
            return Err(CurveError::NonFinite("um vértice do perfil"));
        }
        // A closed profile must go counter-clockwise in (right, up), so the swept tube faces
        // outward. Authoring it the other way round would silently turn the mesh inside out.
        let mut points = points;
        if closed && signed_area(&points) < 0.0 {
            points.reverse();
        }
        Ok(Self { points, closed })
    }

    /// A flat strip `width` meters wide, with sides `thickness` meters tall. Zero thickness gives
    /// a single surface (what a road looks like from above).
    pub fn ribbon(width: f32, thickness: f32) -> Result<Self, CurveError> {
        let half = validate_width("a largura do perfil", width)? * 0.5;
        let points = if thickness > 0.0 {
            if !thickness.is_finite() {
                return Err(CurveError::NonFinite("a espessura do perfil"));
            }
            vec![
                Vec2::new(-half, 0.0),
                Vec2::new(half, 0.0),
                Vec2::new(half, -thickness),
                Vec2::new(-half, -thickness),
            ]
        } else {
            vec![Vec2::new(-half, 0.0), Vec2::new(half, 0.0)]
        };
        Profile::new(points, thickness > 0.0)
    }

    /// A channel `width` wide and `depth` deep, with banks at the edges.
    pub fn trough(width: f32, depth: f32) -> Result<Self, CurveError> {
        let half = validate_width("a largura do canal", width)? * 0.5;
        if !depth.is_finite() || depth <= 0.0 {
            return Err(CurveError::DegenerateProfile(
                "a profundidade do canal precisa ser maior que zero",
            ));
        }
        let quarter = half * 0.5;
        Profile::new(
            vec![
                Vec2::new(-half, 0.0),
                Vec2::new(-quarter, -depth),
                Vec2::new(quarter, -depth),
                Vec2::new(half, 0.0),
            ],
            false,
        )
    }

    /// A closed ring of `sides` vertices at `radius` meters from the center.
    pub fn tube(radius: f32, sides: usize) -> Result<Self, CurveError> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err(CurveError::DegenerateProfile(
                "o raio do tubo precisa ser maior que zero",
            ));
        }
        if !(3..=MAX_PROFILE_POINTS).contains(&sides) {
            return Err(CurveError::TooManyPoints {
                count: sides,
                maximum: MAX_PROFILE_POINTS,
            });
        }
        let mut points = Vec::with_capacity(sides);
        for side in 0..sides {
            let angle = side as f32 / sides as f32 * std::f32::consts::TAU;
            points.push(Vec2::new(angle.cos() * radius, angle.sin() * radius));
        }
        Profile::new(points, true)
    }

    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    pub fn closed(&self) -> bool {
        self.closed
    }
}

/// Twice the signed area of the polygon (positive when counter-clockwise).
fn signed_area(points: &[Vec2]) -> f32 {
    let mut total = 0.0;
    for (index, point) in points.iter().enumerate() {
        let next = points
            .get((index + 1) % points.len())
            .copied()
            .unwrap_or(Vec2::ZERO);
        total += point.x * next.y - next.x * point.y;
    }
    total * 0.5
}

fn validate_width(what: &'static str, width: f32) -> Result<f32, CurveError> {
    if !width.is_finite() || width <= 0.0 {
        return Err(CurveError::DegenerateProfile(what));
    }
    Ok(width)
}
