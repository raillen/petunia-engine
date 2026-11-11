use super::*;

/// An axis-aligned box from `low` to `high`, faces counter-clockwise seen from outside.
fn cuboid(low: Vec3, high: Vec3) -> PolygonMesh {
    let corner = |x: bool, y: bool, z: bool| {
        Vec3::new(
            if x { high.x } else { low.x },
            if y { high.y } else { low.y },
            if z { high.z } else { low.z },
        )
    };
    PolygonMesh {
        positions: vec![
            corner(false, false, false),
            corner(true, false, false),
            corner(true, true, false),
            corner(false, true, false),
            corner(false, false, true),
            corner(true, false, true),
            corner(true, true, true),
            corner(false, true, true),
        ],
        faces: vec![
            vec![0, 3, 2, 1], // back (-z)
            vec![4, 5, 6, 7], // front (+z)
            vec![0, 4, 7, 3], // left (-x)
            vec![1, 2, 6, 5], // right (+x)
            vec![0, 1, 5, 4], // bottom (-y)
            vec![3, 7, 6, 2], // top (+y)
        ],
    }
}

fn unit_box() -> PolygonMesh {
    cuboid(Vec3::splat(-1.0), Vec3::splat(1.0))
}

/// Enclosed volume (divergence theorem over the triangles).
fn volume(mesh: &PolygonMesh) -> f32 {
    mesh.triangles()
        .iter()
        .map(|[a, b, c]| {
            let (a, b, c) = (
                mesh.positions[*a as usize],
                mesh.positions[*b as usize],
                mesh.positions[*c as usize],
            );
            a.dot(b.cross(c)) / 6.0
        })
        .sum()
}

fn close(value: f32, expected: f32) -> bool {
    (value - expected).abs() < 1.0e-3
}

#[test]
fn the_test_box_is_a_closed_solid_of_volume_eight() {
    let mesh = unit_box();
    assert!(is_solid(&mesh));
    assert!(close(volume(&mesh), 8.0), "{}", volume(&mesh));
}

#[test]
fn subtracting_a_box_from_a_box_leaves_a_solid_with_the_right_volume() {
    let wall = cuboid(Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 3.0, 0.5));
    let door = cuboid(Vec3::new(1.5, -1.0, -1.0), Vec3::new(2.5, 2.0, 1.5));
    let result = boolean(&wall, &door, BooleanOperation::Subtract).expect("subtract");
    assert!(is_solid(&result));
    // 4 * 3 * 0.5 minus the doorway 1 * 2 * 0.5.
    assert!(close(volume(&result), 6.0 - 1.0), "{}", volume(&result));
}

#[test]
fn union_and_intersection_follow_inclusion_exclusion() {
    let first = unit_box();
    let second = cuboid(Vec3::splat(0.0), Vec3::splat(2.0));
    let union = boolean(&first, &second, BooleanOperation::Union).expect("union");
    let common = boolean(&first, &second, BooleanOperation::Intersect).expect("intersect");
    assert!(is_solid(&union) && is_solid(&common));
    assert!(close(volume(&common), 1.0), "{}", volume(&common));
    assert!(close(volume(&union), 8.0 + 8.0 - 1.0), "{}", volume(&union));
    // Coplanar triangles were merged back: the intersection is a plain box.
    assert_eq!(common.faces.len(), 6, "{:?}", common.faces);
}

#[test]
fn booleans_refuse_open_meshes_and_results_that_vanish() {
    let mut open = unit_box();
    open.faces.pop();
    let error = boolean(&open, &unit_box(), BooleanOperation::Union).expect_err("open");
    assert_eq!(error.code, "MESH_NOT_SOLID");
    let far = cuboid(Vec3::splat(10.0), Vec3::splat(11.0));
    let error = boolean(&unit_box(), &far, BooleanOperation::Intersect).expect_err("empty");
    assert_eq!(error.code, "MESH_EMPTY_RESULT");
    let swallow = cuboid(Vec3::splat(-2.0), Vec3::splat(2.0));
    let error = boolean(&unit_box(), &swallow, BooleanOperation::Subtract).expect_err("gone");
    assert_eq!(error.code, "MESH_EMPTY_RESULT");
    let error = boolean(&unit_box(), &far, BooleanOperation::Subtract).expect_err("untouched");
    assert_eq!(error.code, "MESH_UNCHANGED");
}

#[test]
fn converting_brush_triangles_welds_and_merges_coplanar_faces() {
    // The drawing mesh of a box: 24 separate vertices (one normal each), 12 triangles.
    let source = unit_box();
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    for face in &source.faces {
        let base = positions.len() as u32;
        for vertex in face {
            positions.push(source.positions[*vertex as usize]);
        }
        triangles.push([base, base + 1, base + 2]);
        triangles.push([base, base + 2, base + 3]);
    }
    let mesh = from_triangles(&positions, &triangles).expect("convert");
    assert_eq!(mesh.positions.len(), 8);
    assert_eq!(mesh.faces.len(), 6);
    assert!(mesh.faces.iter().all(|face| face.len() == 4));
    assert!(is_solid(&mesh));
}

#[test]
fn hostile_input_is_refused_with_a_reason_and_never_panics() {
    let mesh = unit_box();
    let mut broken = mesh.clone();
    broken.positions[0].x = f32::NAN;
    let mut out_of_range = mesh.clone();
    out_of_range.faces[0][0] = 99;
    for bad in [&broken, &out_of_range, &PolygonMesh::default()] {
        assert!(boolean(bad, &mesh, BooleanOperation::Union).is_err());
        assert!(boolean(&mesh, bad, BooleanOperation::Union).is_err());
    }
    assert!(from_triangles(&broken.positions, &[[0, 1, 2]]).is_err());
    assert!(from_triangles(&[], &[]).is_err());
}
