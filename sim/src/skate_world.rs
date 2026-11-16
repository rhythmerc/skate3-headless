//! Collision for authored .skate worlds: the map's static geometry (or its embedded retail
//! RWCM archive) as the board's query world. Drawing the map is the host's (skate-game).
use skate_core::{
    math::Vector3,
    physics::{
        board_world::{
            BoardWorld, WorldTriangle,
            query_metadata::{Bounds, QueryMesh, QueryMetadata, QueryPool},
        },
        collision::TriangleFeature,
        contact::RetailContactMaterial,
        drive_frames::RetailAffineTransform,
    },
};
use glam::Vec3;
use skate_data::skate_map::SkateMap;
use tracing::info_span;
#[allow(unused_imports)]
use std::collections::HashMap;

pub fn validate_runtime(map: &SkateMap) -> Result<(), String> {
    let _span = info_span!("validate_map").entered();
    let archive = retail_archive(map)?;
    if map.geometry.collision.is_empty() && archive.is_none() {
        return Err("SKATE map has no collision geometry".into());
    }
    for triangle in map.geometry.collision.iter().filter(|_| archive.is_none()) {
        if let Some(edges) = triangle.native_edges {
            decode_native_edges(edges)?;
        }
    }
    if !map.doors.is_empty() {
        return Err(format!(
            "Map '{}' contains {} hinged doors. This imported game has no door body/controller adapter yet; refusing to drop their geometry or turn them into static walls.",
            map.name,
            map.doors.len()
        ));
    }
    for extension in &map.extensions {
        let tag = String::from_utf8_lossy(&extension.tag);
        if extension.tag == *b"RWCM" {
            continue;
        }
        if extension.tag == *b"MOBJ" {
            skate_data::skate_map::validate_static_objects(map, extension)?;
            continue;
        }
        if extension.tag == *b"SKYB" && extension.schema == 1 {
            eprintln!(
                "SKATE LIMITATION: SKYB retail sky retained; using the map horizon until its shader adapter is available."
            );
            continue;
        }
        if extension.tag != *b"WMET" && extension.tag != *b"WCFG" && extension.tag != *b"BMAT" {
            return Err(format!(
                "SKATE extension {tag} schema {} is decoded but its runtime adapter is not implemented. Refusing to silently omit potentially required world geometry.",
                extension.schema
            ));
        }
        eprintln!(
            "SKATE LIMITATION: {tag} extension retained; its runtime behavior is not connected."
        );
    }
    if map.materials.iter().any(|m| m.retail_definition.is_some()) {
        eprintln!(
            "SKATE_RENDER: retail world material adapter enabled; unsupported families retain portable PBR rendering."
        );
    }
    if map.textures.iter().any(|t| t.width == 0) {
        return Err(
            "SKATE contains external texture placeholders; supply a package with embedded textures"
                .into(),
        );
    }
    if !map.routes.is_empty() {
        eprintln!(
            "SKATE LIMITATION: {} NPC routes parsed; supplied game has no NPC controller.",
            map.routes.len()
        );
    }
    if map.lights.iter().any(|l| l.kind == 2) {
        eprintln!(
            "SKATE LIMITATION: area-light records retained; Bevy adapter currently renders point and spot lights only."
        );
    }
    eprintln!(
        "SKATE LIMITATION: native frame-lighting/day-night controller is not connected; using package lighting and retail district sky where available."
    );
    eprintln!(
        "SKATE_MAP_LOADED name={:?} version={} render_triangles={} collision_triangles={} textures={} spawn={:?}",
        map.name,
        map.version,
        map.geometry.indices.len() / 3,
        map.geometry.collision.len(),
        map.textures.len(),
        map.spawn
    );
    Ok(())
}

/// TU3 ClusteredMesh::GetUnitVolumes (82AC8A68): fdivs then fsubs,
/// using the pi-squared word at 822F88D0. This is not acos/angle decoding.
/// Bit 7 denotes an unmatched compiler edge and is not a triangle flag.
fn decode_native_edges(edges: [u8; 3]) -> Result<(u32, [f32; 3]), String> {
    let mut flags = 1 | TriangleFeature::ONE_SIDED | TriangleFeature::USE_EDGE_COSINES;
    let mut cosines = [0.; 3];
    for (i, code) in edges.into_iter().enumerate() {
        let exponent = code & 0x1f;
        // The native signed 32-bit shift becomes negative at 28 and zero
        // above it. Reject those malformed codes instead of producing NaNs.
        if exponent >= 28 {
            return Err(format!(
                "Invalid SKATE native edge angle code {exponent} at corner {i}"
            ));
        }
        cosines[i] = 1.0 - f32::from_bits(0x411d_e9e7) / ((8_u32 << exponent) as f32);
        flags |= u32::from(code & 0x20) << i;
        flags |= u32::from(code & 0x40) << (i + 3);
    }
    Ok((flags, cosines))
}

fn retail_archive(map: &SkateMap) -> Result<Option<&[u8]>, String> {
    let mut archives = map.extensions.iter().filter(|e| e.tag == *b"RWCM");
    let Some(archive) = archives.next() else {
        return Ok(None);
    };
    if archive.schema != 1 || archives.next().is_some() {
        return Err("SKATE requires one RWCM extension with schema 1".into());
    }
    Ok(Some(&archive.payload))
}

fn retail_collision_world(
    archive: &[u8],
    material: RetailContactMaterial,
) -> Result<BoardWorld, String> {
    let mut triangles = Vec::new();
    let mut packed_surfaces = Vec::new();
    let mut meshes = Vec::new();
    let count = skate_data::retail_collision::visit_clusters(archive, |_, cluster| {
        // Preserve cluster order and partition further only for group filters.
        let mut cursor = 0;
        while cursor < cluster.len() {
            let group = cluster[cursor].group;
            let start = triangles.len();
            while cursor < cluster.len() && cluster[cursor].group == group {
                let source = cluster[cursor];
                let (flags, cosines) = match source.edges {
                    Some(edges) => {
                        let (mut flags, cosines) = decode_native_edges(edges)?;
                        flags &= !TriangleFeature::ONE_SIDED;
                        if source.one_sided {
                            flags |= TriangleFeature::ONE_SIDED;
                        }
                        (flags, cosines)
                    }
                    // TriangleVolume ctor82AC7770 retains these defaults when
                    // the unit has no edge data; the mesh sidedness is not read.
                    None => (0x1e1, [-1.; 3]),
                };
                triangles.push(
                    WorldTriangle::from_vertices(
                        source.points.map(|p| Vector3::new(p[0], p[1], p[2])),
                        material,
                        u32::from(source.surface),
                        flags,
                        cosines,
                        0.,
                    )
                    .ok_or("Invalid RWCM collision triangle")?,
                );
                packed_surfaces.push(source.surface);
                cursor += 1;
            }
            let range = start..triangles.len();
            let bounds = Bounds::from_points(
                triangles[range.clone()]
                    .iter()
                    .flat_map(|t| t.triangle.vertices),
            )
            .ok_or("Invalid RWCM cluster bounds")?;
            meshes.push(QueryMesh {
                geometry: 0,
                rejection_flags: 0,
                triangle_range: range,
                local_to_world: RetailAffineTransform::IDENTITY,
                world_to_local: RetailAffineTransform::IDENTITY,
                local_bounds: bounds,
                // Static registration `sub_82776B58` hands the mesh-record ctor
                // `sub_8276CB18` a hardcoded `li r8,-1` for matchingID (+180),
                // alongside the `li r7,0` / `li r6,0` for the rejection mask and
                // support identity that the two fields above already reproduce.
                // Every call site of that ctor passes a constant -1 except one,
                // and that one reads its id from a registered actor/unit object
                // (+40) -- never from cluster data. The cluster's packed unit
                // group is a *per-triangle* attribute in retail: `sub_82AC8A68`
                // stores it at triangle+84, right next to the surface code at
                // +88 that `packed_surfaces` above carries. Filing it on the
                // mesh made `matches()` reject every actor whose id was not the
                // cluster's, silently hiding static geometry from offboard
                // ground and line queries.
                matching_group: -1,
                pool: QueryPool::Ground,
            });
        }
        Ok(())
    })?;
    eprintln!(
        "SKATE_RWCM_READY triangles={count} query_clusters={} source=embedded",
        meshes.len()
    );
    BoardWorld::with_query_metadata(
        triangles,
        QueryMetadata {
            packed_surfaces,
            meshes,
            static_edges: vec![],
            island_flags: 0,
        },
    )
    .map_err(str::to_owned)
}

pub fn collision_world(
    map: &SkateMap,
    material: RetailContactMaterial,
) -> Result<BoardWorld, String> {
    if let Some(archive) = retail_archive(map)? {
        return retail_collision_world(archive, material);
    }
    // Match the reference RW mesh compiler's 1 mm vertex welding and reversed
    // edge pairing. Triangle diagonals are adjacency, never authored ledges.
    let mut welded = HashMap::<[i64; 3], usize>::new();
    let mut positions = Vec::<Vec3>::new();
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    for tri in &map.geometry.collision {
        let ids = tri.points.map(|p| {
            let inverse = 1.0 / f64::from(0.001_f32);
            let key = p.map(|v| (f64::from(v) * inverse).round() as i64);
            *welded.entry(key).or_insert_with(|| {
                let id = positions.len();
                positions.push(Vec3::from_array(p));
                id
            })
        });
        let [a, b, c] = tri.points.map(Vec3::from_array);
        let normal = (b - a)
            .cross(c - a)
            .try_normalize()
            .ok_or("Invalid SKATE collision triangle normal")?;
        vertices.push(ids);
        normals.push(normal);
    }
    let mut cosines = vec![[1.; 3]; vertices.len()];
    let mut flags =
        vec![TriangleFeature::ONE_SIDED | TriangleFeature::USE_EDGE_COSINES | 0xe0; vertices.len()];
    // Fully native maps need no reconstructed adjacency. Mixed maps still
    // include every face when finding neighbors for their authored geometry.
    if map
        .geometry
        .collision
        .iter()
        .any(|t| t.native_edges.is_none())
    {
        let mut open = HashMap::<(usize, usize), (usize, usize)>::new();
        for (i, ids) in vertices.iter().enumerate() {
            for edge in 0..3 {
                let (a, b) = (ids[edge], ids[(edge + 1) % 3]);
                if let Some((other, oe)) = open.remove(&(b, a)) {
                    let cosine = normals[i].dot(normals[other]).clamp(-1., 1.);
                    let orientation =
                        (positions[b] - positions[a]).dot(normals[i].cross(normals[other]));
                    // ExtendedEdgeCosine / MakeEdgeCode in rw_collision_mesh.cpp:
                    // orientation >= -1e-6 is convex; flat edges have no convex bit.
                    for (ti, e) in [(i, edge), (other, oe)] {
                        cosines[ti][e] = cosine;
                        if orientation <= -1.0e-6 || cosine >= 1. {
                            flags[ti] &= !(0x20 << e);
                        }
                    }
                } else {
                    open.entry((a, b)).or_insert((i, edge));
                }
            }
        }
        let mut adjacent = vec![Vec::new(); positions.len()];
        for (i, ids) in vertices.iter().enumerate() {
            for &v in ids {
                adjacent[v].push(i);
            }
        }
        for (v, faces) in adjacent.iter().enumerate() {
            let reference = normals[faces[0]];
            if faces
                .iter()
                .all(|&i| (reference.dot(normals[i]) - 1.).abs() <= 0.01)
            {
                for &i in faces {
                    for corner in 0..3 {
                        if vertices[i][corner] == v {
                            flags[i] |= 0x200 << corner;
                        }
                    }
                }
            }
        }
    }
    let mut triangles = Vec::with_capacity(vertices.len());
    let mut packed_surfaces = Vec::with_capacity(vertices.len());
    for (i, source) in map.geometry.collision.iter().enumerate() {
        if let Some(edges) = source.native_edges {
            (flags[i], cosines[i]) = decode_native_edges(edges)?;
        }
        let m = &map.materials[source.material as usize - 1];
        // Exact EncodeRwSurfaceId mapping from the reference native adapter.
        packed_surfaces.push((m.audio | (m.physics << 7) | (m.pattern << 12)) as u16);
        let points = vertices[i].map(|id| {
            let p = positions[id];
            Vector3::new(p.x, p.y, p.z)
        });
        // Keep the supplied game's original static-world contact combine values.
        // The native map bridge supplies packed surfaces, not a guessed split of
        // the package's single friction scalar into static/dynamic coefficients.
        triangles.push(
            WorldTriangle::from_vertices(
                points,
                material,
                source.surface,
                flags[i],
                cosines[i],
                0.,
            )
            .ok_or("Invalid SKATE collision volume")?,
        );
    }
    // Portable maps have no native cluster hierarchy. Bound contiguous ranges
    // once at load time so the existing BVH can reject distant geometry. Keep
    // triangle order and mesh identity/filter values: contact tie-breaking,
    // packed surfaces and adjacency must not change with this acceleration.
    let mut meshes = Vec::new();
    for start in (0..triangles.len()).step_by(64) {
        let end = (start + 64).min(triangles.len());
        let bounds = Bounds::from_points(
            triangles[start..end]
                .iter()
                .flat_map(|t| t.triangle.vertices),
        )
        .ok_or("SKATE collision bounds empty")?;
        meshes.push(QueryMesh {
            geometry: 0,
            rejection_flags: 0,
            triangle_range: start..end,
            local_to_world: RetailAffineTransform::IDENTITY,
            world_to_local: RetailAffineTransform::IDENTITY,
            local_bounds: bounds,
            matching_group: -1,
            pool: QueryPool::Ground,
        });
    }
    let metadata = QueryMetadata {
        packed_surfaces,
        meshes,
        static_edges: vec![],
        island_flags: 0,
    };
    BoardWorld::with_query_metadata(triangles, metadata).map_err(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn compare_cluster_queries(world: &BoardWorld) -> (usize, usize) {
        let mut metadata = world.query_metadata().unwrap().clone();
        let mut single = metadata.meshes[0].clone();
        single.triangle_range = 0..world.triangles().len();
        single.local_bounds =
            Bounds::from_points(world.triangles().iter().flat_map(|t| t.triangle.vertices))
                .unwrap();
        metadata.meshes = vec![single];
        let baseline =
            BoardWorld::with_query_metadata(world.triangles().to_vec(), metadata).unwrap();
        let mut visited = [0usize; 2];
        let mut times = [std::time::Duration::ZERO; 2];
        for triangle in world
            .triangles()
            .iter()
            .step_by((world.triangles().len() / 256).max(1))
        {
            let p = triangle.triangle.vertices[0];
            let start = Vector3::new(p.x, p.y + 1., p.z);
            let end = Vector3::new(p.x, p.y - 1., p.z);
            let mut candidates = Vec::new();
            for (i, scene) in [&baseline, world].into_iter().enumerate() {
                let bounds = scene.line_candidate_bounds(start, end, 0.1);
                visited[i] += scene
                    .candidate_ranges(bounds)
                    .iter()
                    .map(|r| r.len())
                    .sum::<usize>();
                let now = std::time::Instant::now();
                candidates.push(
                    scene
                        .line_candidates(start, end, 0.1)
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>(),
                );
                times[i] += now.elapsed();
            }
            assert_eq!(
                candidates[0], candidates[1],
                "candidate identity/order changed"
            );
        }
        eprintln!(
            "CUSTOM_QUERY_BENCH baseline_triangles={} clustered_triangles={} baseline_ms={} clustered_ms={} clusters={}",
            visited[0],
            visited[1],
            times[0].as_secs_f64() * 1000.,
            times[1].as_secs_f64() * 1000.,
            world.query_metadata().unwrap().meshes.len()
        );
        (visited[0], visited[1])
    }

    #[test]
    fn portable_collision_clusters_preserve_queries_and_cull_distant_geometry() {
        let mut map = demo();
        let source = map.geometry.collision.remove(0);
        map.geometry.collision.clear();
        for i in 0..256 {
            map.geometry
                .collision
                .push(skate_data::skate_map::Collision {
                    points: source.points.map(|p| [p[0] + i as f32 * 100., p[1], p[2]]),
                    surface: source.surface,
                    material: source.material,
                    native_edges: source.native_edges,
                });
        }
        let world = collision_world(&map, material()).unwrap();
        assert_eq!(world.query_metadata().unwrap().meshes.len(), 4);
        let (before, after) = compare_cluster_queries(&world);
        assert!(after < before / 2);
    }

    #[test]
    #[ignore = "requires a supplied map; static collision-query benchmark only"]
    fn supplied_custom_map_collision_query_benchmark() {
        let path = std::env::var("SKATE_TEST_CUSTOM_MAP").unwrap();
        let map = SkateMap::load(std::path::Path::new(&path)).unwrap();
        let world = collision_world(&map, material()).unwrap();
        let (before, after) = compare_cluster_queries(&world);
        assert!(after < before);
    }
    fn demo() -> SkateMap {
        SkateMap::parse(include_bytes!("../tests/fixtures/format-demo.skate")).unwrap()
    }
    fn material() -> RetailContactMaterial {
        RetailContactMaterial {
            static_friction: 0.,
            dynamic_friction: 0.,
            restitution: 1.,
        }
    }
    #[test]
    fn native_edges_override_generated_adjacency() {
        let mut map = demo();
        map.geometry.collision[0].native_edges = Some([0x20, 0x42, 0x9a]);
        validate_runtime(&map).unwrap();
        let world = collision_world(&map, material()).unwrap();
        let f = world.triangles()[0].triangle.feature;
        assert!(f.edge_convex(0));
        assert!(!f.edge_convex(1));
        assert!(!f.edge_convex(2));
        assert!(!f.vertex_disabled(0));
        assert!(f.vertex_disabled(1));
        assert!(!f.vertex_disabled(2));
        assert_eq!(f.edge_cosines[0].to_bits(), 0xbe6f4f38);
        assert_eq!(f.edge_cosines[1].to_bits(), 0x3f310b0c);
        assert_eq!(f.edge_cosines[2], 1.);
        assert!(decode_native_edges([31, 0, 0]).is_err());
        assert_eq!(decode_native_edges([0, 2, 26]).unwrap().1, f.edge_cosines);
    }
    #[test]
    fn embedded_archive_is_authoritative_and_preserves_cluster_metadata() {
        let mut map = demo();
        map.geometry.collision.clear();
        map.extensions.push(skate_data::skate_map::Extension {
            tag: *b"RWCM",
            schema: 1,
            payload: include_bytes!("../../skate-data/tests/fixtures/retail-collision.rwcmset")
                .to_vec(),
        });
        validate_runtime(&map).unwrap();
        let world = collision_world(&map, material()).unwrap();
        assert_eq!(world.triangles().len(), 3);
        let metadata = world.query_metadata().unwrap();
        assert_eq!(metadata.meshes.len(), 3);
        // The cluster group still partitions the clusters into meshes (three
        // here), but it must not land on the mesh's matchingID: retail's static
        // registration writes -1 there. See the comment at the construction
        // site. This is what `embedded_static_rwcm_hits_distinct_actor_query_ids`
        // depends on, and the two tests disagreed until 2026-09-20.
        assert!(metadata.meshes.iter().all(|m| m.matching_group == -1));
        assert_eq!(metadata.packed_surfaces, vec![0x4321; 3]);
        assert_eq!(world.triangles()[1].triangle.vertices[0].x, 10.);
    }
    #[test]
    fn native_unit_without_edge_data_keeps_constructor_defaults() {
        let mut archive =
            include_bytes!("../../skate-data/tests/fixtures/retail-collision.rwcmset").to_vec();
        let name_len = u32::from_le_bytes(archive[12..16].try_into().unwrap()) as usize;
        let cluster = 16 + name_len + 4 + 160;
        archive[cluster + 2..cluster + 4].copy_from_slice(&8_u16.to_be_bytes());
        archive[cluster + 80] = 0xc1;
        archive[cluster + 84..cluster + 88].copy_from_slice(&[0x34, 0x12, 0x21, 0x43]);
        let world = retail_collision_world(&archive, material()).unwrap();
        let f = world.triangles()[0].triangle.feature;
        assert_eq!(f.flags, 0x1e1);
        assert_eq!(f.edge_cosines, [-1.; 3]);
    }
    #[test]
    #[ignore = "requires SKATE_MAP_TEST_PATH pointing to a private map"]
    fn private_map_builds_collision_world() {
        let path = std::env::var("SKATE_MAP_TEST_PATH").unwrap();
        let map = SkateMap::load(std::path::Path::new(&path)).unwrap();
        validate_runtime(&map).unwrap();
        let world = collision_world(&map, material()).unwrap();
        let [x, y, z] = map.spawn;
        let hit = world
            .query_thin_line(Vector3::new(x, y + 1., z), Vector3::new(x, y - 10., z))
            .unwrap();
        assert!(hit.is_some(), "spawn has no supporting collision");
        eprintln!(
            "Private map world: {} triangles, spawn hit {:?}",
            world.triangles().len(),
            hit
        );
    }
    #[test]
    fn map_collision_uses_separate_geometry_and_surface_metadata() {
        let mut map = demo();
        map.geometry.vertices[0].position = [1000.; 3];
        map.materials[0].audio = 42;
        map.materials[0].physics = 4;
        map.materials[0].pattern = 7;
        let world = collision_world(&map, material()).unwrap();
        assert_eq!(world.triangles().len(), 2);
        assert_eq!(
            world.triangles()[0].triangle.vertices[0],
            Vector3::new(-30., 0., -30.)
        );
        assert!(world.triangles()[0].triangle.feature.normal.y > 0.999);
        assert_eq!(
            world.query_metadata().unwrap().packed_surfaces,
            vec![42 | (4 << 7) | (7 << 12); 2]
        );
        assert!(world.query_metadata().unwrap().static_edges.is_empty());
    }
    #[test]
    fn shared_flat_diagonal_is_not_a_convex_contact_edge() {
        let world = collision_world(&demo(), material()).unwrap();
        let a = world.triangles()[0].triangle.feature;
        let b = world.triangles()[1].triangle.feature;
        assert!(!a.edge_convex(2));
        assert!(!b.edge_convex(0));
        assert!(a.edge_convex(0));
        assert_eq!(a.edge_cosines[2], 1.);
        assert!(a.vertex_disabled(0));
    }
}
