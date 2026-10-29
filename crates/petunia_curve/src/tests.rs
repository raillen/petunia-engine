use petunia_math::{PolygonMesh, Vec3};

use super::*;

fn line(count: usize) -> Vec<Vec3> {
    (0..count)
        .map(|index| Vec3::new(0.0, 0.0, index as f32 * 10.0))
        .collect()
}

fn straight() -> Spline {
    Spline::through(&line(3), false).expect("spline")
}

fn closed_ring() -> Spline {
    // A square in the XZ plane, closed.
    Spline::through(
        &[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 10.0),
        ],
        true,
    )
    .expect("anel")
}

// ---- the curve ------------------------------------------------------------------------------

#[test]
fn a_spline_needs_two_finite_points() {
    assert!(matches!(
        Spline::through(&[], false),
        Err(CurveError::TooFewPoints(0))
    ));
    assert!(matches!(
        Spline::new(vec![SplinePoint::corner(Vec3::ZERO)], false),
        Err(CurveError::TooFewPoints(1))
    ));
    let many: Vec<Vec3> = (0..257)
        .map(|index| Vec3::new(index as f32, 0.0, 0.0))
        .collect();
    assert!(matches!(
        Spline::through(&many, false),
        Err(CurveError::TooManyPoints { .. })
    ));
    assert!(matches!(
        Spline::through(&[Vec3::ZERO, Vec3::new(f32::NAN, 0.0, 0.0)], false),
        Err(CurveError::NonFinite(_))
    ));
}

#[test]
fn the_ends_of_the_curve_are_the_first_and_last_points() {
    let spline = straight();
    assert_eq!(spline.evaluate(0.0), Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(spline.evaluate(1.0), Vec3::new(0.0, 0.0, 20.0));
    // Out of range is clamped, not extrapolated.
    assert_eq!(spline.evaluate(-3.0), Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(spline.evaluate(4.0), Vec3::new(0.0, 0.0, 20.0));
    // The middle of a straight spline is in the middle.
    assert!((spline.evaluate(0.5).z - 10.0).abs() < 0.001);
}

#[test]
fn a_closed_spline_joins_the_last_point_back_to_the_first() {
    let ring = closed_ring();
    assert_eq!(ring.segment_count(), 4);
    assert_eq!(ring.evaluate(0.0), Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(ring.evaluate(1.0), Vec3::new(0.0, 0.0, 0.0));
    let half = ring.evaluate(0.5);
    assert!(
        (half.x - 10.0).abs() < 0.5 && (half.z - 10.0).abs() < 0.5,
        "{half:?}"
    );
}

#[test]
fn the_length_of_a_straight_curve_is_the_distance_between_the_ends() {
    let spline = straight();
    assert!((spline.length() - 20.0).abs() < 0.01, "{}", spline.length());
    let ring = closed_ring();
    // A 10 m square is 40 m around; the smooth handles round the corners outward, so the curve
    // is a little longer than the polygon but nowhere near twice.
    let length = ring.length();
    assert!((40.0..46.0).contains(&length), "{length}");
}

#[test]
fn handles_are_absolute_and_a_corner_turns_sharply() {
    // Two segments that meet at a right angle with no handles: a corner.
    let spline = Spline::new(
        vec![
            SplinePoint::corner(Vec3::new(0.0, 0.0, 0.0)),
            SplinePoint::corner(Vec3::new(10.0, 0.0, 0.0)),
            SplinePoint::corner(Vec3::new(10.0, 0.0, 10.0)),
        ],
        false,
    )
    .expect("spline");
    // In the middle of the first segment the curve is on the straight line between the points.
    let middle = spline.evaluate(0.25);
    assert!((middle.z).abs() < 0.001, "{middle:?}");
    assert!((middle.x - 5.0).abs() < 0.001, "{middle:?}");
    // The corner has a direction even without handles.
    let tangent = spline.tangent(0.5);
    assert!(tangent.length() > 0.9);
}

#[test]
fn a_smooth_spline_bends_instead_of_turning() {
    let spline = Spline::through(
        &[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(20.0, 0.0, 0.0),
        ],
        false,
    )
    .expect("spline");
    // The handle of the middle point follows the chord between its neighbours, so the curve
    // reaches beyond the straight line between the first and last point in z.
    let middle = spline.evaluate(0.5);
    assert!(middle.z > 5.0, "{middle:?}");
}

#[test]
fn bounds_cover_every_control_point() {
    let spline = Spline::through(
        &[
            Vec3::new(-2.0, 1.0, 4.0),
            Vec3::new(6.0, -3.0, 1.0),
            Vec3::new(0.0, 2.0, -8.0),
        ],
        false,
    )
    .expect("spline");
    let (minimum, maximum) = spline.bounds().expect("limites");
    assert_eq!(minimum, Vec3::new(-2.0, -3.0, -8.0));
    assert_eq!(maximum, Vec3::new(6.0, 2.0, 4.0));
}

// ---- the frames -----------------------------------------------------------------------------

#[test]
fn frames_are_orthonormal_and_evenly_spaced() {
    let spline = straight();
    let frames = spline.frames(5).expect("quadros");
    assert_eq!(frames.len(), 5);
    assert_eq!(frames[0].position, Vec3::new(0.0, 0.0, 0.0));
    assert!((frames[4].position.z - 20.0).abs() < 0.001);
    for frame in &frames {
        assert!((frame.tangent.length() - 1.0).abs() < 0.001);
        assert!((frame.normal.length() - 1.0).abs() < 0.001);
        assert!((frame.binormal.length() - 1.0).abs() < 0.001);
        assert!(frame.tangent.dot(frame.normal).abs() < 0.001);
        assert!(frame.tangent.dot(frame.binormal).abs() < 0.001);
        assert!(frame.normal.dot(frame.binormal).abs() < 0.001);
    }
    // Equal distance: the gaps between consecutive frames are all 5 m.
    for pair in frames.windows(2) {
        let gap = (pair[1].position - pair[0].position).length();
        assert!((gap - 5.0).abs() < 0.01, "{gap}");
    }
}

#[test]
fn the_frame_does_not_flip_upside_down_on_an_s_curve() {
    let spline = Spline::through(
        &[
            Vec3::new(-20.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 8.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, -8.0),
            Vec3::new(60.0, 0.0, 0.0),
        ],
        false,
    )
    .expect("spline");
    let frames = spline.frames(40).expect("quadros");
    for pair in frames.windows(2) {
        // Parallel transport: the "up" axis never jumps to the other side.
        assert!(
            pair[0].binormal.dot(pair[1].binormal) > 0.8,
            "{:?} -> {:?}",
            pair[0].binormal,
            pair[1].binormal
        );
    }
}

#[test]
fn a_frame_on_a_curve_that_goes_straight_up_still_exists() {
    let spline = Spline::through(
        &[Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0)],
        false,
    )
    .expect("spline");
    let frames = spline.frames(3).expect("quadros");
    for frame in frames {
        assert!(frame.normal.length() > 0.99);
        assert!(frame.binormal.length() > 0.99);
    }
}

#[test]
fn a_curve_without_direction_is_rejected_instead_of_panicking() {
    let spline = Spline::through(&[Vec3::ZERO, Vec3::ZERO], false).expect("dois pontos");
    assert!(matches!(
        spline.frames(4),
        Err(CurveError::Degenerate(_)) | Err(CurveError::InvalidSteps(_))
    ));
    assert!(matches!(spline.frames(0), Err(CurveError::InvalidSteps(0))));
}

// ---- the profile ----------------------------------------------------------------------------

#[test]
fn profiles_cannot_be_empty_or_unbounded() {
    assert!(matches!(
        Profile::new(vec![], false),
        Err(CurveError::TooFewPoints(0))
    ));
    assert!(matches!(
        Profile::ribbon(0.0, 0.0),
        Err(CurveError::DegenerateProfile(_))
    ));
    assert!(matches!(
        Profile::trough(4.0, 0.0),
        Err(CurveError::DegenerateProfile(_))
    ));
    assert!(matches!(
        Profile::tube(0.0, 8),
        Err(CurveError::DegenerateProfile(_))
    ));
    assert!(matches!(
        Profile::tube(1.0, 2),
        Err(CurveError::TooManyPoints { .. })
    ));
    assert!(matches!(
        Profile::new(vec![petunia_math::Vec2::new(f32::INFINITY, 0.0); 2], false),
        Err(CurveError::NonFinite(_))
    ));
}

#[test]
fn a_closed_profile_is_turned_counter_clockwise() {
    // Written clockwise (0,0) → (0,1) → (1,1) → (1,0): the constructor reverses it.
    let profile = Profile::new(
        vec![
            petunia_math::Vec2::new(0.0, 0.0),
            petunia_math::Vec2::new(0.0, 1.0),
            petunia_math::Vec2::new(1.0, 1.0),
            petunia_math::Vec2::new(1.0, 0.0),
        ],
        true,
    )
    .expect("perfil");
    let points = profile.points();
    assert_eq!(points[0], petunia_math::Vec2::new(1.0, 0.0));
    assert_eq!(points[3], petunia_math::Vec2::new(0.0, 0.0));
}

#[test]
fn the_built_in_profiles_have_the_size_they_were_asked_for() {
    let ribbon = Profile::ribbon(4.0, 0.0).expect("faixa");
    assert_eq!(ribbon.points().len(), 2);
    assert_eq!(ribbon.points()[0].x, -2.0);
    assert!(!ribbon.closed());

    let road = Profile::ribbon(4.0, 0.2).expect("faixa com espessura");
    assert_eq!(road.points().len(), 4);
    assert!(road.closed());
    for point in road.points() {
        assert!(point.x.abs() <= 2.0 + f32::EPSILON);
        assert!(point.y <= 0.0 + f32::EPSILON);
    }

    let trough = Profile::trough(6.0, 1.5).expect("calha");
    let lowest = trough
        .points()
        .iter()
        .map(|point| point.y)
        .fold(0.0_f32, f32::min);
    assert!((lowest + 1.5).abs() < 0.001);

    let tube = Profile::tube(0.5, 8).expect("tubo");
    assert!(tube.closed());
    assert_eq!(tube.points().len(), 8);
    for point in tube.points() {
        let radius = (point.x * point.x + point.y * point.y).sqrt();
        assert!((radius - 0.5).abs() < 0.001);
    }
}

#[test]
fn profile_kinds_round_trip_by_name() {
    for kind in [ProfileKind::Ribbon, ProfileKind::Trough, ProfileKind::Tube] {
        assert_eq!(ProfileKind::parse(kind.name()), Some(kind));
        kind.build(4.0, 1.0, 0.0, 8).expect("constrói");
    }
    assert_eq!(ProfileKind::parse("faixa"), Some(ProfileKind::Ribbon));
    assert_eq!(ProfileKind::parse("calha"), Some(ProfileKind::Trough));
    assert_eq!(ProfileKind::parse("tubo"), Some(ProfileKind::Tube));
    assert_eq!(ProfileKind::parse("fita"), None);
}

// ---- the sweep ------------------------------------------------------------------------------

fn sweep_straight(profile: Profile, steps: usize, cap_ends: bool) -> PolygonMesh {
    let spline = straight();
    sweep(&spline, &profile, &SweepSettings { steps, cap_ends }).expect("varredura")
}

#[test]
fn sweeping_a_straight_line_keeps_the_profile_width() {
    let mesh = sweep_straight(Profile::ribbon(4.0, 0.0).expect("faixa"), 5, true);
    assert_eq!(mesh.positions.len(), 5 * 2);
    // 4 strips between 5 rings, each a quad.
    assert_eq!(mesh.faces.len(), 4);
    assert!(mesh.faces.iter().all(|face| face.len() == 4));
    let (minimum, maximum) = mesh.bounds().expect("limites");
    assert!((maximum.x - minimum.x - 4.0).abs() < 0.01);
    // The ribbon follows the curve: 20 m long in z.
    assert!((maximum.z - minimum.z - 20.0).abs() < 0.01);
    // A flat surface: no height.
    assert!((maximum.y - minimum.y).abs() < 0.001);
    mesh.validate().expect("malha válida");
}

#[test]
fn a_ribbon_faces_up_and_a_tube_faces_outward() {
    let ribbon = sweep_straight(Profile::ribbon(4.0, 0.0).expect("faixa"), 3, false);
    for face in &ribbon.faces {
        let normal = ribbon.face_normal(face);
        assert!(normal.y > 0.9, "faixa virada para baixo: {normal:?}");
    }
    let tube = sweep_straight(Profile::tube(1.0, 8).expect("tubo"), 3, false);
    for face in &tube.faces {
        let normal = tube.face_normal(face);
        let center = face
            .iter()
            .map(|vertex| tube.positions[*vertex as usize])
            .fold(Vec3::ZERO, |sum, position| sum + position)
            * (1.0 / face.len() as f32);
        // The face normal points away from the curve, which runs through the middle.
        assert!(normal.dot(center) > 0.0, "tubo do avesso: {normal:?}");
    }
}

#[test]
fn capping_only_closes_a_closed_profile() {
    let open = sweep_straight(Profile::ribbon(4.0, 0.0).expect("faixa"), 4, true);
    assert_eq!(open.faces.len(), 3);
    let tube = sweep_straight(Profile::tube(1.0, 8).expect("tubo"), 4, true);
    // 3 rings of 8 quads, plus two caps.
    assert_eq!(tube.faces.len(), 3 * 8 + 2);
    let uncapped = sweep_straight(Profile::tube(1.0, 8).expect("tubo"), 4, false);
    assert_eq!(uncapped.faces.len(), 3 * 8);
}

#[test]
fn a_trough_sweeps_a_channel_below_the_curve() {
    let mesh = sweep_straight(Profile::trough(6.0, 1.5).expect("calha"), 4, false);
    let (minimum, maximum) = mesh.bounds().expect("limites");
    assert!(maximum.y.abs() < 0.001, "as bordas ficam na curva");
    assert!((minimum.y + 1.5).abs() < 0.001, "o fundo desce 1,5 m");
    assert!((maximum.x - minimum.x - 6.0).abs() < 0.01);
}

#[test]
fn the_sweep_is_deterministic() {
    let spline = closed_ring();
    let profile = Profile::tube(0.4, 6).expect("tubo");
    let settings = SweepSettings {
        steps: 16,
        cap_ends: true,
    };
    let first = sweep(&spline, &profile, &settings).expect("varredura");
    let second = sweep(&spline, &profile, &settings).expect("varredura");
    assert_eq!(first, second);
    assert_eq!(first.positions.len(), 16 * 6);
    first.validate().expect("malha válida");
}

#[test]
fn the_sweep_rejects_bad_settings() {
    let spline = straight();
    let profile = Profile::ribbon(2.0, 0.0).expect("faixa");
    assert!(matches!(
        sweep(
            &spline,
            &profile,
            &SweepSettings {
                steps: 0,
                cap_ends: false
            }
        ),
        Err(CurveError::InvalidSteps(0))
    ));
    assert!(matches!(
        sweep(
            &spline,
            &profile,
            &SweepSettings {
                steps: 513,
                cap_ends: false
            }
        ),
        Err(CurveError::InvalidSteps(513))
    ));
}

#[test]
fn one_ring_is_a_mesh_without_faces() {
    let mesh = sweep_straight(Profile::ribbon(2.0, 0.0).expect("faixa"), 1, true);
    assert_eq!(mesh.positions.len(), 2);
    assert!(mesh.faces.is_empty());
    mesh.validate().expect("malha válida");
}
