use petunia_math::Vec3;

use crate::error::TerrainError;

/// Shape of a brush. All shapes are volumes in world space, so a brush can carve from any camera
/// angle, including from outside the cave (DR-33).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrushShape {
    Sphere { radius: f32 },
    Box { half_extents: Vec3 },
    Cylinder { radius: f32, half_height: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl BrushShape {
    /// Distance from `point` to the brush center, normalized so 0 is the center and 1 is the
    /// brush boundary. Values above 1 are outside the brush.
    pub fn normalized_distance(self, center: Vec3, point: Vec3) -> f32 {
        let offset = point - center;
        match self {
            Self::Sphere { radius } => offset.length() / radius.max(0.001),
            Self::Box { half_extents } => {
                let x = offset.x.abs() / half_extents.x.max(0.001);
                let y = offset.y.abs() / half_extents.y.max(0.001);
                let z = offset.z.abs() / half_extents.z.max(0.001);
                x.max(y).max(z)
            }
            Self::Cylinder {
                radius,
                half_height,
            } => {
                let horizontal = Vec3::new(offset.x, 0.0, offset.z).length();
                (horizontal / radius.max(0.001)).max(offset.y.abs() / half_height.max(0.001))
            }
            Self::Capsule {
                radius,
                half_height,
            } => {
                let start = center - Vec3::UP * half_height;
                let end = center + Vec3::UP * half_height;
                segment_distance(point, start, end) / radius.max(0.001)
            }
        }
    }

    /// Axis-aligned bounds of the volume in world space.
    pub fn bounds(self, center: Vec3) -> (Vec3, Vec3) {
        let half = match self {
            Self::Sphere { radius } => Vec3::splat(radius),
            Self::Box { half_extents } => half_extents,
            Self::Cylinder {
                radius,
                half_height,
            }
            | Self::Capsule {
                radius,
                half_height,
            } => Vec3::new(radius, half_height, radius),
        };
        (center - half, center + half)
    }

    pub fn validate(self) -> Result<(), TerrainError> {
        let positive = |value: f32, what: &'static str| -> Result<(), TerrainError> {
            if !value.is_finite() || value <= 0.0 {
                Err(TerrainError::InvalidBrush(what))
            } else {
                Ok(())
            }
        };
        match self {
            Self::Sphere { radius } => positive(radius, "o raio precisa ser positivo"),
            Self::Box { half_extents } => {
                positive(half_extents.x, "a meia-largura em x precisa ser positiva")?;
                positive(half_extents.y, "a meia-altura em y precisa ser positiva")?;
                positive(
                    half_extents.z,
                    "a meia-profundidade em z precisa ser positiva",
                )
            }
            Self::Cylinder {
                radius,
                half_height,
            }
            | Self::Capsule {
                radius,
                half_height,
            } => {
                positive(radius, "o raio precisa ser positivo")?;
                positive(half_height, "a meia-altura precisa ser positiva")
            }
        }
    }
}

/// How the brush strength fades from the center to the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falloff {
    /// Full strength inside the volume, nothing outside.
    Constant,
    Linear,
    Smooth,
    Gaussian,
}

impl Falloff {
    /// 1 at the center, 0 at the boundary, smooth in between.
    pub fn weight(self, normalized_distance: f32) -> f32 {
        let distance = normalized_distance;
        if !distance.is_finite() || distance >= 1.0 {
            return 0.0;
        }
        match self {
            Self::Constant => 1.0,
            Self::Linear => (1.0 - distance).clamp(0.0, 1.0),
            Self::Smooth => {
                let inside = (1.0 - distance).clamp(0.0, 1.0);
                inside * inside * (3.0 - 2.0 * inside)
            }
            Self::Gaussian => {
                let raw = (-4.0 * distance * distance).exp();
                let edge = (-4.0f32).exp();
                ((raw - edge) / (1.0 - edge)).clamp(0.0, 1.0)
            }
        }
    }
}

/// A 3D brush: where it acts, how far it reaches and how strong it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub shape: BrushShape,
    pub center: Vec3,
    pub falloff: Falloff,
    /// Meters for `Raise`/`Lower`/`Noise`, blend in `0..=1` for the other operations.
    pub strength: f32,
}

impl Brush {
    pub fn new(shape: BrushShape, center: Vec3, falloff: Falloff, strength: f32) -> Self {
        Self {
            shape,
            center,
            falloff,
            strength,
        }
    }

    pub fn validate(&self) -> Result<(), TerrainError> {
        self.shape.validate()?;
        if !self.center.is_finite() {
            return Err(TerrainError::NonFinite("o centro do pincel"));
        }
        if !self.strength.is_finite() || self.strength < 0.0 {
            return Err(TerrainError::InvalidBrush(
                "a força precisa ser um número finito maior ou igual a zero",
            ));
        }
        Ok(())
    }

    pub fn normalized_distance(&self, point: Vec3) -> f32 {
        self.shape.normalized_distance(self.center, point)
    }

    pub fn weight(&self, point: Vec3) -> f32 {
        self.falloff.weight(self.normalized_distance(point))
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        self.shape.bounds(self.center)
    }
}

/// What a brush does to the height samples under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TerrainOperation {
    Raise,
    Lower,
    Smooth,
    /// Moves samples toward a plane. `None` uses the height at the brush center.
    Flatten {
        target_height: Option<f32>,
    },
    Noise {
        seed: u64,
        feature_size: f32,
        amplitude: f32,
    },
    Terrace {
        step: f32,
    },
}

fn segment_distance(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let along = end - start;
    let from_start = point - start;
    let length_squared = along.length_squared();
    let parameter = if length_squared > 0.0 {
        (from_start.dot(along) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (from_start - along * parameter).length()
}
