use super::*;
use petunia_math::Vec3;

fn flat_terrain(cells: usize, cell_size: f32) -> Terrain {
    let mut terrain = Terrain::new(cell_size, cells).unwrap();
    terrain.ensure_chunk(ChunkCoord::new(0, 0), 0.0).unwrap();
    terrain
}

fn sphere_brush(center: Vec3, radius: f32, falloff: Falloff, strength: f32) -> Brush {
    Brush::new(BrushShape::Sphere { radius }, center, falloff, strength)
}

#[test]
fn rejects_invalid_settings() {
    assert!(Terrain::new(0.0, 16).is_err());
    assert!(Terrain::new(0.001, 16).is_err());
    assert!(Terrain::new(1.0, 0).is_err());
    assert!(Terrain::new(1.0, 257).is_err());
    assert!(Terrain::new(f32::NAN, 16).is_err());
}

#[test]
fn generation_is_deterministic() {
    let settings = TerrainGeneration {
        fractal: FractalSettings {
            seed: 42,
            feature_size: 32.0,
            amplitude: 10.0,
            octaves: 4,
            lacunarity: 2.0,
            persistence: 0.5,
        },
        base_height: 2.0,
    };
    let mut first = Terrain::new(1.0, 16).unwrap();
    let mut second = Terrain::new(1.0, 16).unwrap();
    first.generate(ChunkCoord::new(0, 0), &settings).unwrap();
    second.generate(ChunkCoord::new(0, 0), &settings).unwrap();
    assert_eq!(first, second);
    assert!(first.chunk_count() == 1);
}

#[test]
fn generated_height_matches_the_settings() {
    let settings = TerrainGeneration {
        fractal: FractalSettings {
            seed: 7,
            feature_size: 20.0,
            amplitude: 8.0,
            octaves: 3,
            lacunarity: 2.0,
            persistence: 0.5,
        },
        base_height: -1.0,
    };
    let mut terrain = Terrain::new(1.0, 16).unwrap();
    terrain.generate(ChunkCoord::new(0, 0), &settings).unwrap();
    let sampled = terrain.sample(3, 4).unwrap();
    let expected = settings.height_at(3.0, 4.0);
    assert!((sampled - expected).abs() < 1.0e-5);
}

#[test]
fn generation_rejects_bad_parameters() {
    let settings = TerrainGeneration {
        fractal: FractalSettings {
            amplitude: -1.0,
            ..FractalSettings::default()
        },
        ..TerrainGeneration::default()
    };
    assert!(settings.validate().is_err());
    let mut terrain = Terrain::new(1.0, 16).unwrap();
    assert!(terrain.generate(ChunkCoord::new(0, 0), &settings).is_err());
}

#[test]
fn raise_lifts_only_the_brush_area() {
    let mut terrain = flat_terrain(16, 1.0);
    let brush = sphere_brush(Vec3::new(8.0, 0.0, 8.0), 3.0, Falloff::Smooth, 2.0);
    let changed = terrain.apply(&brush, TerrainOperation::Raise).unwrap();
    assert!(changed > 0);
    assert!((terrain.sample(8, 8).unwrap() - 2.0).abs() < 1.0e-4);
    assert_eq!(terrain.sample(0, 0).unwrap(), 0.0);
    assert_eq!(terrain.sample(12, 8).unwrap(), 0.0);
}

#[test]
fn lower_digs_only_the_brush_area() {
    let mut terrain = flat_terrain(16, 1.0);
    let brush = sphere_brush(Vec3::new(8.0, 0.0, 8.0), 3.0, Falloff::Linear, 1.5);
    terrain.apply(&brush, TerrainOperation::Lower).unwrap();
    assert!((terrain.sample(8, 8).unwrap() + 1.5).abs() < 1.0e-4);
    assert_eq!(terrain.sample(0, 0).unwrap(), 0.0);
}

#[test]
fn smooth_pulls_a_spike_down() {
    let mut terrain = flat_terrain(16, 1.0);
    terrain.set_sample(8, 8, 10.0).unwrap();
    let brush = sphere_brush(Vec3::new(8.0, 10.0, 8.0), 2.0, Falloff::Constant, 1.0);
    terrain.apply(&brush, TerrainOperation::Smooth).unwrap();
    assert!(terrain.sample(8, 8).unwrap() < 10.0);
}

#[test]
fn flatten_moves_toward_the_plane() {
    let mut terrain = flat_terrain(16, 1.0);
    terrain.set_sample(8, 8, 5.0).unwrap();
    let brush = sphere_brush(Vec3::new(8.0, 5.0, 8.0), 2.0, Falloff::Constant, 1.0);
    terrain
        .apply(
            &brush,
            TerrainOperation::Flatten {
                target_height: Some(2.0),
            },
        )
        .unwrap();
    assert!((terrain.sample(8, 8).unwrap() - 2.0).abs() < 1.0e-4);
}

#[test]
fn terrace_snaps_to_steps() {
    let mut terrain = flat_terrain(16, 1.0);
    terrain.set_sample(8, 8, 4.7).unwrap();
    let brush = sphere_brush(Vec3::new(8.0, 4.7, 8.0), 2.0, Falloff::Constant, 1.0);
    terrain
        .apply(&brush, TerrainOperation::Terrace { step: 1.0 })
        .unwrap();
    assert!((terrain.sample(8, 8).unwrap() - 5.0).abs() < 1.0e-4);
}

#[test]
fn noise_is_deterministic_and_changes_the_surface() {
    let operation = TerrainOperation::Noise {
        seed: 99,
        feature_size: 8.0,
        amplitude: 1.0,
    };
    let brush = sphere_brush(Vec3::new(8.0, 0.0, 8.0), 4.0, Falloff::Smooth, 1.0);
    let mut first = flat_terrain(16, 1.0);
    let mut second = flat_terrain(16, 1.0);
    first.apply(&brush, operation).unwrap();
    second.apply(&brush, operation).unwrap();
    assert_eq!(first, second);
    assert_ne!(first.sample(8, 8).unwrap(), 0.0);
}

#[test]
fn box_brush_uses_chebyshev_distance() {
    let mut terrain = flat_terrain(16, 1.0);
    let brush = Brush::new(
        BrushShape::Box {
            half_extents: Vec3::new(3.0, 3.0, 3.0),
        },
        Vec3::new(8.0, 0.0, 8.0),
        Falloff::Constant,
        1.0,
    );
    terrain.apply(&brush, TerrainOperation::Raise).unwrap();
    assert!((terrain.sample(8, 8).unwrap() - 1.0).abs() < 1.0e-4);
    assert!((terrain.sample(10, 10).unwrap() - 1.0).abs() < 1.0e-4);
    assert_eq!(terrain.sample(12, 8).unwrap(), 0.0);
}

#[test]
fn cylinder_and_capsule_are_supported() {
    for shape in [
        BrushShape::Cylinder {
            radius: 2.0,
            half_height: 2.0,
        },
        BrushShape::Capsule {
            radius: 2.0,
            half_height: 3.0,
        },
    ] {
        let mut terrain = flat_terrain(16, 1.0);
        let brush = Brush::new(shape, Vec3::new(8.0, 0.0, 8.0), Falloff::Constant, 1.0);
        terrain.apply(&brush, TerrainOperation::Raise).unwrap();
        assert!(terrain.sample(8, 8).unwrap() > 0.0);
        assert_eq!(terrain.sample(0, 0).unwrap(), 0.0);
    }
}

#[test]
fn falloff_weights_go_from_one_to_zero() {
    assert!((Falloff::Constant.weight(0.5) - 1.0).abs() < 1.0e-6);
    assert!((Falloff::Linear.weight(0.5) - 0.5).abs() < 1.0e-6);
    assert!((Falloff::Smooth.weight(0.5) - 0.5).abs() < 1.0e-6);
    assert!((Falloff::Gaussian.weight(0.0) - 1.0).abs() < 1.0e-6);
    assert_eq!(Falloff::Gaussian.weight(1.0), 0.0);
    assert_eq!(Falloff::Linear.weight(2.0), 0.0);
}

#[test]
fn height_at_interpolates_bilinearly() {
    let mut terrain = flat_terrain(16, 1.0);
    terrain.set_sample(0, 0, 0.0).unwrap();
    terrain.set_sample(1, 0, 1.0).unwrap();
    terrain.set_sample(0, 1, 2.0).unwrap();
    terrain.set_sample(1, 1, 3.0).unwrap();
    let middle = terrain.height_at(0.5, 0.5).unwrap();
    assert!((middle - 1.5).abs() < 1.0e-5);
}

#[test]
fn height_across_a_chunk_border_is_continuous() {
    let mut terrain = Terrain::new(1.0, 4).unwrap();
    for (x, z, height) in [(3, 3, 0.0), (4, 3, 1.0), (3, 4, 2.0), (4, 4, 3.0)] {
        terrain.set_sample(x, z, height).unwrap();
    }
    let middle = terrain.height_at(3.5, 3.5).unwrap();
    assert!((middle - 1.5).abs() < 1.0e-5);
    assert!(terrain.chunk(ChunkCoord::new(1, 1)).is_some());
}

#[test]
fn negative_samples_use_negative_chunks() {
    let mut terrain = Terrain::new(1.0, 4).unwrap();
    terrain.set_sample(-1, -1, 7.0).unwrap();
    assert!((terrain.sample(-1, -1).unwrap() - 7.0).abs() < 1.0e-6);
    assert!(terrain.chunk(ChunkCoord::new(-1, -1)).is_some());
    assert!(terrain.height_at(-0.5, -0.5).is_none());
}

#[test]
fn normal_is_up_on_flat_ground() {
    let terrain = flat_terrain(16, 1.0);
    let normal = terrain.normal_at(8.0, 8.0).unwrap();
    assert!((normal.y - 1.0).abs() < 1.0e-5);
    assert!(normal.x.abs() < 1.0e-5);
    assert!(normal.z.abs() < 1.0e-5);
}

#[test]
fn invalid_brushes_are_rejected() {
    let mut terrain = flat_terrain(16, 1.0);
    let zero_radius = sphere_brush(Vec3::ZERO, 0.0, Falloff::Smooth, 1.0);
    assert!(
        terrain
            .apply(&zero_radius, TerrainOperation::Raise)
            .is_err()
    );
    let bad_center = sphere_brush(Vec3::new(f32::NAN, 0.0, 0.0), 1.0, Falloff::Smooth, 1.0);
    assert!(terrain.apply(&bad_center, TerrainOperation::Raise).is_err());
    let negative_strength = sphere_brush(Vec3::ZERO, 1.0, Falloff::Smooth, -1.0);
    assert!(
        terrain
            .apply(&negative_strength, TerrainOperation::Raise)
            .is_err()
    );
}

#[test]
fn a_brush_covering_too_many_samples_is_rejected() {
    let terrain = Terrain::new(0.05, 16).unwrap();
    let brush = sphere_brush(Vec3::ZERO, 5_000.0, Falloff::Smooth, 1.0);
    let mut terrain = terrain;
    assert!(terrain.apply(&brush, TerrainOperation::Raise).is_err());
}

#[test]
fn samples_are_clamped_and_must_be_finite() {
    let mut terrain = flat_terrain(16, 1.0);
    terrain.set_sample(8, 8, MAX_HEIGHT + 500.0).unwrap();
    assert!((terrain.sample(8, 8).unwrap() - MAX_HEIGHT).abs() < 1.0e-3);
    assert!(terrain.set_sample(8, 8, f32::INFINITY).is_err());
}

#[test]
fn capture_and_restore_undo_a_stroke() {
    let mut terrain = flat_terrain(8, 1.0);
    let before = terrain.capture_chunk(ChunkCoord::new(0, 0));
    let brush = sphere_brush(Vec3::new(4.0, 0.0, 4.0), 2.0, Falloff::Smooth, 1.0);
    terrain.apply(&brush, TerrainOperation::Raise).unwrap();
    assert_ne!(terrain.sample(4, 4).unwrap(), 0.0);
    terrain
        .restore_chunk(ChunkCoord::new(0, 0), before)
        .unwrap();
    assert_eq!(terrain.sample(4, 4).unwrap(), 0.0);
}

#[test]
fn capture_region_restores_every_chunk() {
    let mut terrain = Terrain::new(1.0, 4).unwrap();
    let region = terrain.capture_region(ChunkCoord::new(0, 0), ChunkCoord::new(1, 1));
    terrain.set_sample(0, 0, 3.0).unwrap();
    terrain.set_sample(4, 4, 5.0).unwrap();
    assert_eq!(terrain.chunk_count(), 2);
    terrain.restore_region(region).unwrap();
    assert_eq!(terrain.chunk_count(), 0);
}

#[test]
fn generate_region_builds_every_chunk() {
    let mut terrain = Terrain::new(1.0, 8).unwrap();
    terrain
        .generate_region(
            ChunkCoord::new(0, 0),
            ChunkCoord::new(1, 1),
            &TerrainGeneration::default(),
        )
        .unwrap();
    assert_eq!(terrain.chunk_count(), 4);
    assert!(terrain.bounds().is_some());
}

#[test]
fn materials_can_be_painted_and_read() {
    let mut terrain = flat_terrain(8, 1.0);
    terrain.set_sample_material(2, 3, 7).unwrap();
    assert_eq!(terrain.sample_material(2, 3), Some(7));
}

#[test]
fn brush_chunk_region_matches_the_bounds() {
    let terrain = Terrain::new(1.0, 4).unwrap();
    let brush = sphere_brush(Vec3::new(5.0, 0.0, 5.0), 2.0, Falloff::Smooth, 1.0);
    let (minimum, maximum) = terrain.brush_chunk_region(&brush);
    assert_eq!(minimum, ChunkCoord::new(0, 0));
    assert_eq!(maximum, ChunkCoord::new(1, 1));
}

#[test]
fn chunks_can_be_rebuilt_from_saved_parts() {
    let chunk =
        TerrainChunk::from_parts(ChunkCoord::new(1, -2), 4, vec![1.0; 16], vec![3; 16]).unwrap();
    let mut terrain = Terrain::new(1.0, 4).unwrap();
    terrain.insert_chunk(chunk).unwrap();
    assert_eq!(terrain.sample(4, -8), Some(1.0));
    assert_eq!(terrain.sample_material(4, -8), Some(3));
    assert!(terrain.validate().is_ok());
}

#[test]
fn broken_saved_chunks_are_refused() {
    assert!(TerrainChunk::from_parts(ChunkCoord::new(0, 0), 4, vec![0.0; 15], Vec::new()).is_err());
    assert!(
        TerrainChunk::from_parts(ChunkCoord::new(0, 0), 4, vec![f32::NAN; 16], Vec::new()).is_err()
    );
    assert!(
        TerrainChunk::from_parts(ChunkCoord::new(0, 0), 4, vec![0.0; 16], vec![1; 15]).is_err()
    );
}

#[test]
fn default_terrain_is_empty_and_valid() {
    let terrain = Terrain::default();
    assert!(terrain.is_empty());
    assert_eq!(terrain.sample_count(), 0);
    assert!(terrain.validate().is_ok());
}

#[test]
fn paint_changes_only_existing_samples() {
    let mut terrain = flat_terrain(8, 1.0);
    let brush = sphere_brush(Vec3::new(4.0, 0.0, 4.0), 2.0, Falloff::Smooth, 1.0);
    let painted = terrain.paint(&brush, 5).unwrap();
    assert!(painted > 0);
    assert_eq!(terrain.sample_material(4, 4), Some(5));
    assert_eq!(terrain.sample_material(0, 0), Some(0));
}

#[test]
fn holes_are_validated_and_detected() {
    let mut terrain = flat_terrain(8, 1.0);
    terrain
        .set_holes(vec![TerrainHole {
            x: 2.0,
            z: 2.0,
            radius: 1.0,
        }])
        .unwrap();
    assert!(terrain.is_hole(2.0, 2.0));
    assert!(terrain.is_hole(2.5, 2.5));
    assert!(!terrain.is_hole(4.0, 2.0));
    assert!(terrain.validate().is_ok());
    assert!(
        terrain
            .set_holes(vec![TerrainHole {
                x: f32::NAN,
                z: 0.0,
                radius: 1.0
            }])
            .is_err()
    );
    assert!(
        terrain
            .set_holes(vec![TerrainHole {
                x: 0.0,
                z: 0.0,
                radius: 0.0
            }])
            .is_err()
    );
}

#[test]
fn thermal_erosion_slides_a_spike_down() {
    let mut terrain = flat_terrain(8, 1.0);
    terrain.set_sample(4, 4, 5.0).unwrap();
    let changed = terrain.erode(4, 0.5, 0.5).unwrap();
    assert!(changed > 0);
    assert!(terrain.sample(4, 4).unwrap() < 5.0);
}

#[test]
fn thermal_erosion_is_deterministic() {
    let mut first = flat_terrain(8, 1.0);
    let mut second = flat_terrain(8, 1.0);
    first.set_sample(3, 3, 5.0).unwrap();
    second.set_sample(3, 3, 5.0).unwrap();
    first.erode(3, 0.5, 0.5).unwrap();
    second.erode(3, 0.5, 0.5).unwrap();
    assert_eq!(first, second);
}

#[test]
fn thermal_erosion_rejects_bad_parameters() {
    let mut terrain = flat_terrain(8, 1.0);
    assert!(terrain.erode(0, 0.5, 0.5).is_err());
    assert!(terrain.erode(65, 0.5, 0.5).is_err());
    assert!(terrain.erode(1, f32::NAN, 0.5).is_err());
    assert!(terrain.erode(1, 0.5, 2.0).is_err());
}

#[test]
fn stamp_replaces_a_rectangular_patch() {
    let mut terrain = Terrain::new(1.0, 8).unwrap();
    terrain
        .generate_region(
            ChunkCoord::new(-1, -1),
            ChunkCoord::new(0, 0),
            &TerrainGeneration {
                fractal: FractalSettings {
                    amplitude: 0.0,
                    ..FractalSettings::default()
                },
                base_height: 0.0,
            },
        )
        .unwrap();
    let changed = terrain
        .stamp(Stamp {
            center_x: 0.5,
            center_z: 0.5,
            width: 2,
            depth: 2,
            heights: &[1.0, 2.0, 3.0, 4.0],
            mode: StampMode::Replace,
            strength: 1.0,
        })
        .unwrap();
    assert_eq!(changed, 4);
    assert!((terrain.sample(0, 0).unwrap() - 1.0).abs() < 1.0e-6);
    assert!((terrain.sample(1, 0).unwrap() - 2.0).abs() < 1.0e-6);
    assert!((terrain.sample(0, 1).unwrap() - 3.0).abs() < 1.0e-6);
    assert!((terrain.sample(1, 1).unwrap() - 4.0).abs() < 1.0e-6);
}

#[test]
fn stamp_adds_and_clamps() {
    let mut terrain = flat_terrain(8, 1.0);
    terrain
        .stamp(Stamp {
            center_x: 0.0,
            center_z: 0.0,
            width: 1,
            depth: 1,
            heights: &[2.0],
            mode: StampMode::Add,
            strength: 0.5,
        })
        .unwrap();
    assert!((terrain.sample(0, 0).unwrap() - 1.0).abs() < 1.0e-6);
    terrain
        .stamp(Stamp {
            center_x: 0.0,
            center_z: 0.0,
            width: 1,
            depth: 1,
            heights: &[-3.0],
            mode: StampMode::Min,
            strength: 1.0,
        })
        .unwrap();
    assert!((terrain.sample(0, 0).unwrap() + 3.0).abs() < 1.0e-6);
}

#[test]
fn stamp_rejects_bad_data() {
    let mut terrain = flat_terrain(8, 1.0);
    assert!(
        terrain
            .stamp(Stamp {
                center_x: 0.0,
                center_z: 0.0,
                width: 0,
                depth: 1,
                heights: &[1.0],
                mode: StampMode::Add,
                strength: 1.0
            })
            .is_err()
    );
    assert!(
        terrain
            .stamp(Stamp {
                center_x: 0.0,
                center_z: 0.0,
                width: 2,
                depth: 2,
                heights: &[1.0],
                mode: StampMode::Add,
                strength: 1.0
            })
            .is_err()
    );
    assert!(
        terrain
            .stamp(Stamp {
                center_x: f32::NAN,
                center_z: 0.0,
                width: 1,
                depth: 1,
                heights: &[1.0],
                mode: StampMode::Add,
                strength: 1.0
            })
            .is_err()
    );
    assert!(
        terrain
            .stamp(Stamp {
                center_x: 0.0,
                center_z: 0.0,
                width: 1,
                depth: 1,
                heights: &[1.0],
                mode: StampMode::Add,
                strength: -1.0
            })
            .is_err()
    );
}

#[test]
fn hydraulic_erosion_wears_a_spike_down() {
    let mut terrain = flat_terrain(8, 1.0);
    terrain.set_sample(4, 4, 5.0).unwrap();
    let changed = terrain.hydraulic_erode(500, 0.5, 0.3, 0.02, 7).unwrap();
    assert!(changed > 0);
    assert!(terrain.sample(4, 4).unwrap() < 5.0);
}

#[test]
fn hydraulic_erosion_is_deterministic() {
    let mut first = flat_terrain(8, 1.0);
    let mut second = flat_terrain(8, 1.0);
    first.set_sample(3, 3, 5.0).unwrap();
    second.set_sample(3, 3, 5.0).unwrap();
    first.hydraulic_erode(200, 0.5, 0.3, 0.02, 11).unwrap();
    second.hydraulic_erode(200, 0.5, 0.3, 0.02, 11).unwrap();
    assert_eq!(first, second);
}

#[test]
fn hydraulic_erosion_rejects_bad_parameters() {
    let mut terrain = flat_terrain(8, 1.0);
    assert!(terrain.hydraulic_erode(0, 0.5, 0.3, 0.02, 1).is_err());
    assert!(terrain.hydraulic_erode(100_001, 0.5, 0.3, 0.02, 1).is_err());
    assert!(terrain.hydraulic_erode(10, 2.0, 0.3, 0.02, 1).is_err());
    assert!(terrain.hydraulic_erode(10, 0.5, f32::NAN, 0.02, 1).is_err());
}
