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

fn top_face(mesh: &PolygonMesh) -> u32 {
    (0..mesh.faces.len())
        .max_by(|a, b| {
            let height = |face: usize| mesh.face_normal(&mesh.faces[face]).y;
            height(*a).total_cmp(&height(*b))
        })
        .unwrap_or(0) as u32
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
fn inset_adds_a_ring_and_keeps_the_solid_closed() {
    let mesh = unit_box();
    let top = top_face(&mesh);
    let (result, inner) = inset(&mesh, &[top], 0.25, Grouping::Region).expect("inset");
    assert_eq!(
        result.faces.len(),
        6 + 4,
        "one inner face replaces the top, four ring faces"
    );
    assert_eq!(result.positions.len(), 8 + 4);
    assert!(is_solid(&result));
    assert!(close(volume(&result), 8.0));
    assert_eq!(inner.len(), 1);
    let inner_face = &result.faces[inner[0] as usize];
    for vertex in inner_face {
        let point = result.positions[*vertex as usize];
        assert!(
            close(point.x.abs(), 0.75) && close(point.z.abs(), 0.75),
            "{point:?}"
        );
    }
}

#[test]
fn extruding_the_top_face_raises_it_and_adds_walls() {
    let mesh = unit_box();
    let top = top_face(&mesh);
    let result = extrude(&mesh, &[top], 1.0, Grouping::Region).expect("extrude");
    assert!(is_solid(&result));
    assert_eq!(result.faces.len(), 10);
    assert!(close(volume(&result), 12.0), "{}", volume(&result));
    let dug = extrude(&mesh, &[top], -0.5, Grouping::Region).expect("dig");
    assert!(close(volume(&dug), 6.0), "{}", volume(&dug));
}

#[test]
fn a_loop_cut_splits_the_ring_of_four_side_faces() {
    let mesh = unit_box();
    // Edge 0–1 runs along x at the bottom back; its ring goes around the box through the back,
    // bottom, front and top faces.
    let result = loop_cut(&mesh, [0, 1], 1, 0.0).expect("loop cut");
    assert_eq!(result.faces.len(), 10, "four ring faces become eight");
    assert_eq!(result.positions.len(), 12);
    assert!(is_solid(&result));
    assert!(close(volume(&result), 8.0));
    let two = loop_cut(&mesh, [0, 1], 2, 0.0).expect("two cuts");
    assert_eq!(two.faces.len(), 14);
    let after_inset = inset(&mesh, &[top_face(&mesh)], 0.2, Grouping::Region)
        .expect("inset")
        .0;
    assert!(loop_cut(&after_inset, [0, 1], 1, 0.0).is_ok());
}

#[test]
fn loop_cuts_explain_rings_that_cross_triangles() {
    // A triangular prism: the ring through the side quads closes, but an edge of a cap triangle
    // starts a ring that hits a triangle.
    let prism = PolygonMesh {
        positions: vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(1.0, 0.0, 2.0),
            Vec3::new(0.0, 1.0, 2.0),
        ],
        faces: vec![
            vec![0, 2, 1],
            vec![3, 4, 5],
            vec![0, 1, 4, 3],
            vec![1, 2, 5, 4],
            vec![2, 0, 3, 5],
        ],
    };
    assert!(is_solid(&prism));
    // Along the length: the ring of three side quads closes around the prism.
    let around = loop_cut(&prism, [0, 3], 1, 0.0).expect("around");
    assert_eq!(around.faces.len(), 8);
    assert!(is_solid(&around));
    // Across: the ring starts on a side quad and ends cleanly in the two cap triangles
    // (Petunia3D splits them instead of stopping), and the solid stays closed.
    let across = loop_cut(&prism, [0, 1], 1, 0.0).expect("across");
    assert!(is_solid(&across));
    assert!(close(volume(&across), volume(&prism)));
    // A ring that runs into an n-gon also ends there, and the solid stays closed.
    let mut pentagon = unit_box();
    pentagon.positions.push(Vec3::new(0.0, 1.0, 1.0));
    pentagon.faces[1] = vec![4, 5, 6, 8, 7];
    pentagon.faces[5] = vec![3, 7, 8, 6, 2];
    assert!(is_solid(&pentagon));
    let into_ngon = loop_cut(&pentagon, [0, 3], 1, 0.0).expect("ring into an n-gon");
    assert!(is_solid(&into_ngon));
    assert!(close(volume(&into_ngon), 8.0));
    // An edge on the border of an open mesh has no ring across it on one side; a ring that
    // only touches the border still cuts, and nothing panics.
    let open = delete_faces(&unit_box(), &[5]).expect("open box");
    assert!(loop_cut(&open, [0, 1], 1, 0.0).is_ok_and(|cut| cut.validate().is_ok()));
}

#[test]
fn beveling_an_edge_replaces_it_with_a_strip() {
    let mesh = unit_box();
    let chamfer = bevel(&mesh, &[[3, 2]], 0.2, 1).expect("chamfer");
    assert!(is_solid(&chamfer));
    assert_eq!(chamfer.faces.len(), 7);
    // The cut-off corner prism has legs of 0.2: 0.5 * 0.2 * 0.2 * 2 (length).
    assert!(close(volume(&chamfer), 8.0 - 0.04), "{}", volume(&chamfer));
    let round = bevel(&mesh, &[[3, 2]], 0.2, 4).expect("round");
    assert!(is_solid(&round));
    assert_eq!(round.faces.len(), 6 + 4);
    // Known limit of Petunia3D's bevel: two edges that meet at a corner are refused together,
    // and the mesh does not change.
    let corner = bevel(&mesh, &[[3, 2], [2, 6]], 0.2, 1).expect_err("corner");
    assert_eq!(corner.code, "MESH_BEVEL_UNSUPPORTED");
}

#[test]
fn subdividing_a_quad_makes_a_grid_and_keeps_neighbors_joined() {
    let mesh = unit_box();
    let top = top_face(&mesh);
    let result = subdivide(&mesh, &[top], 1).expect("subdivide");
    assert_eq!(result.faces.len(), 5 + 4);
    assert!(
        is_solid(&result),
        "the side faces gained the new edge points"
    );
    assert!(close(volume(&result), 8.0));
    let error = subdivide(&mesh, &[top], 0).expect_err("zero cuts");
    assert_eq!(error.code, "MESH_BAD_PARAMETER");
}

#[test]
fn dissolve_merge_delete_and_flip() {
    let mesh = unit_box();
    let cut = loop_cut(&mesh, [0, 1], 1, 0.0).expect("loop cut");
    // Dissolving every new edge of the loop brings the box back to six faces.
    let new_vertices: Vec<u32> = (8..12).collect();
    let loop_edges: Vec<[u32; 2]> = cut
        .faces
        .iter()
        .flat_map(|face| {
            (0..face.len()).map(move |corner| [face[corner], face[(corner + 1) % face.len()]])
        })
        .filter(|[a, b]| new_vertices.contains(a) && new_vertices.contains(b))
        .collect();
    let dissolved = dissolve_edges(&cut, &loop_edges).expect("dissolve");
    assert_eq!(dissolved.faces.len(), 6);
    assert!(close(volume(&dissolved), 8.0));

    let merged = merge_vertices(&mesh, &[6, 7]).expect("merge");
    assert_eq!(merged.positions.len(), 7);
    assert!(merged.validate().is_ok());

    let mut doubled = mesh.clone();
    doubled
        .positions
        .push(doubled.positions[0] + Vec3::splat(1.0e-5));
    doubled.faces[0][0] = 8;
    let (welded, removed) = merge_by_distance(&doubled, 1.0e-3).expect("weld");
    assert_eq!(removed, 1);
    assert!(is_solid(&welded));

    let open = delete_faces(&mesh, &[top_face(&mesh)]).expect("delete");
    assert_eq!(open.faces.len(), 5);
    assert!(!is_solid(&open));

    let flipped = flip_faces(&mesh, &[]).expect("flip all");
    assert!(close(volume(&flipped), -8.0));
}

#[test]
fn the_knife_cuts_one_face_in_two() {
    let mesh = unit_box();
    let top = top_face(&mesh) as usize;
    let face = &mesh.faces[top];
    let start = EdgePoint {
        edge: [face[0], face[1]],
        fraction: 0.5,
    };
    let end = EdgePoint {
        edge: [face[2], face[3]],
        fraction: 0.5,
    };
    let result = knife(&mesh, start, end).expect("knife");
    assert_eq!(result.faces.len(), 7);
    assert!(
        is_solid(&result),
        "the new points are shared with the neighbors"
    );
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
        assert!(inset(bad, &[0], 0.1, Grouping::Region).is_err());
        assert!(extrude(bad, &[0], 0.1, Grouping::Region).is_err());
        assert!(loop_cut(bad, [0, 1], 1, 0.0).is_err());
        assert!(bevel(bad, &[[0, 1]], 0.1, 1).is_err());
        assert!(boolean(bad, &mesh, BooleanOperation::Union).is_err());
        assert!(boolean(&mesh, bad, BooleanOperation::Union).is_err());
        assert!(subdivide(bad, &[0], 1).is_err());
        assert!(flip_faces(bad, &[]).is_err());
    }
    for distance in [f32::NAN, f32::INFINITY, 0.0, 1.0e9] {
        assert_eq!(
            inset(&mesh, &[0], distance, Grouping::Region).map(|_| ()),
            Err(inset(&mesh, &[0], distance, Grouping::Region).expect_err("bad"))
        );
        assert!(extrude(&mesh, &[0], distance, Grouping::Region).is_err());
        assert!(bevel(&mesh, &[[0, 1]], distance, 1).is_err());
    }
    assert_eq!(
        inset(&mesh, &[], 0.1, Grouping::Region)
            .expect_err("none")
            .code,
        "MESH_NO_FACES"
    );
    assert_eq!(
        inset(&mesh, &[6], 0.1, Grouping::Region)
            .expect_err("unknown")
            .code,
        "MESH_UNKNOWN_FACE"
    );
    assert_eq!(
        loop_cut(&mesh, [0, 6], 1, 0.0).expect_err("diagonal").code,
        "MESH_UNKNOWN_EDGE"
    );
    assert!(loop_cut(&mesh, [0, 1], 33, 0.0).is_err());
    assert!(loop_cut(&mesh, [0, 1], 1, 1.5).is_err());
    assert!(bevel(&mesh, &[[0, 1]], 0.1, 99).is_err());
    assert!(merge_vertices(&mesh, &[0]).is_err());
    assert!(merge_vertices(&mesh, &[0, 99]).is_err());
    assert_eq!(
        delete_faces(&mesh, &[0, 1, 2, 3, 4, 5])
            .expect_err("all")
            .code,
        "MESH_EMPTY_RESULT"
    );
    let far_inset = inset(&mesh, &[top_face(&mesh)], 5.0, Grouping::Region);
    assert!(
        far_inset.is_ok() || far_inset.is_err(),
        "a huge inset never panics"
    );
}

#[test]
fn every_operation_is_deterministic() {
    let mesh = unit_box();
    let top = top_face(&mesh);
    let other = cuboid(Vec3::splat(0.0), Vec3::splat(2.0));
    let run = || {
        vec![
            boolean(&mesh, &other, BooleanOperation::Subtract),
            boolean(&mesh, &other, BooleanOperation::Union),
            inset(&mesh, &[top, 0], 0.2, Grouping::Individual).map(|(result, _)| result),
            extrude(&mesh, &[top, 3], 0.5, Grouping::Region),
            loop_cut(&mesh, [0, 1], 3, 0.25),
            bevel(&mesh, &[[3, 2], [4, 5]], 0.2, 3),
            subdivide(&mesh, &[top, 1], 2),
        ]
    };
    let first = run();
    for _ in 0..3 {
        assert_eq!(run(), first);
    }
    assert!(first.iter().all(Result::is_ok), "{first:?}");
}

#[test]
fn the_solid_check_agrees_with_petunia3d_half_edges() {
    let joined = |first: &PolygonMesh, second: &PolygonMesh| {
        let mut both = first.clone();
        let offset = both.positions.len() as u32;
        both.positions.extend(second.positions.iter().copied());
        both.faces.extend(
            second
                .faces
                .iter()
                .map(|face| face.iter().map(|vertex| vertex + offset).collect()),
        );
        both
    };
    let mut open = unit_box();
    open.faces.pop();
    let mut one_flipped = unit_box();
    one_flipped.faces[2].reverse();
    // Two boxes far apart: two closed pieces in one mesh.
    let apart = joined(&unit_box(), &cuboid(Vec3::splat(5.0), Vec3::splat(6.0)));
    // A box with an extra face hanging on one edge (three faces on that edge).
    let mut fin = unit_box();
    fin.positions.push(Vec3::new(0.0, 3.0, -1.0));
    fin.faces.push(vec![3, 2, 8]);
    let inset_box = inset(&unit_box(), &[5], 0.2, Grouping::Region)
        .expect("inset")
        .0;
    for (label, mesh, expected) in [
        ("box", unit_box(), true),
        ("open", open, false),
        ("one face inside out", one_flipped, false),
        ("two pieces", apart, true),
        ("fin on an edge", fin, false),
        ("inset", inset_box, true),
    ] {
        let legacy_answer = legacy::HalfEdgeMesh::from_mesh(&to_legacy(&mesh))
            .0
            .is_closed_manifold();
        assert_eq!(is_solid(&mesh), expected, "{label}");
        assert_eq!(legacy_answer, expected, "Petunia3D on {label}");
    }
}

#[test]
fn subdividing_part_of_a_dense_mesh_stays_closed() {
    let all: Vec<u32> = (0..6).collect();
    let dense = subdivide(&unit_box(), &all, 6).expect("dense");
    assert!(is_solid(&dense));
    // Every other face of the dense box: neighbors of every subdivided face get its new points.
    let half: Vec<u32> = (0..dense.faces.len() as u32).step_by(2).collect();
    let finer = subdivide(&dense, &half, 2).expect("half");
    assert!(
        is_solid(&finer),
        "no crack between subdivided and untouched faces"
    );
    assert!(close(volume(&finer), 8.0));
}
