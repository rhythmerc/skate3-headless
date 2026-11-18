//! The skater's model and pose as the contract's ([`engine_core::model`]).
//!
//! - **Model** ([`model`]): the manifest's GLB, its skin's joints named as the stock rig's bones, and the rig's
//!   parents. The GLB was exported through Blender: its bone-local axes are turned -90 degrees about x from the rig's
//!   own ([`BASIS`]; both are y-up in the world). That is a skin basis change, not a physics turn. Its bodies: the
//!   rider (cast from its bones) and the board (its footprint, truck to truck).
//! - **Pose** ([`pose`]): the sim's render pose (the rig's bones in the animation root's space) and that root in the
//!   world (the contract's: `crate::stage::FLOOR`), published once a tick. It is a cut (not blended from the one before) when the gameplay camera cuts or the
//!   sim's teleport flag is up.
use engine_core::glam::{Mat4, Vec3, Vec4};
use engine_core::model::{BodyGroup, Cast, Model, ModelSource, Point, Pose};
use engine_core::skate::body;
use skate_core::camera::CameraFrame;
use skate3_sim::physics::SkaterRuntime;

/// A bone's matrix in the GLB's axes: the rig's times this (`x, -z, y`).
// (-Vec4::Z, as the app always built it: its zeros negative, so products round exactly as they did)
pub const BASIS: Mat4 = Mat4::from_cols(Vec4::X, Vec4::new(-0.0, -0.0, -1.0, -0.0), Vec4::Y, Vec4::W);

/// The skater's model: the GLB at `character_scene` (the manifest's), bound to the sim's rig.
pub fn model(character_scene: &str, skater: &SkaterRuntime) -> Model {
    let frames = &skater.animation.evaluator.frames;
    let parents: Vec<Option<u16>> = frames.parents.iter().map(|&p| u16::try_from(p).ok()).collect();
    let groups = groups(&parents, skater.skeleton_output.pose.board_bones.all(), skater.skeleton_output.pose.board_bones.back_truck);
    Model {
        source: ModelSource::Gltf { path: character_scene.to_owned(), joint_names: frames.bone_names.clone(), basis: BASIS },
        parents,
        groups,
    }
}

/// The rig's bodies: the board (`board`: its trucks and wheels, the front truck first; with their parent, the board's
/// root), cast as [`body::BOARD_FOOTPRINT`] from the back truck to the front one; and the rider, cast from its bones:
/// every bone but those, the rig's root (the trajectory, on the ground) and the root's other children (the hands and
/// toes reparented to the board for its IK, which follow the rider's own).
fn groups(parents: &[Option<u16>], board: [usize; 6], back_truck: usize) -> Vec<BodyGroup> {
    let parent = |i: usize| parents.get(i).copied().flatten().map(usize::from);
    let Some(board_root) = parent(board[0]) else { return Vec::new() };
    let mut on_board: Vec<u16> = std::iter::once(board_root).chain(board).filter_map(|i| u16::try_from(i).ok()).collect();
    on_board.sort_unstable();
    on_board.dedup();
    let rider: Vec<u16> = (0..parents.len())
        .filter(|&i| !on_board.contains(&(i as u16)) && parent(i).is_some_and(|p| p != board_root))
        .filter_map(|i| u16::try_from(i).ok())
        .collect();
    let point = |i: usize| Point { joint: i as u16, at: Vec3::ZERO };
    vec![
        BodyGroup { id: body::RIDER, joints: rider, cast: Cast::Segments },
        BodyGroup {
            id: body::BOARD,
            joints: on_board,
            cast: Cast::Footprint { back: point(back_truck), front: point(board[0]), ellipses: body::BOARD_FOOTPRINT.to_vec() },
        },
    ]
}

/// The skater's pose after the last tick, with the camera's frame of it (`camera`).
pub fn pose(skater: &SkaterRuntime, camera: &CameraFrame, out: &mut Pose) {
    let pose = skate3_sim::SkaterPose::of(skater);
    out.generation = pose.generation;
    out.discontinuity = camera.discontinuity || skater.player_input.processed.flags_2472 & (1 << 10) != 0;
    out.root = crate::stage::frame_from_sim(pose.root);
    out.joints.clear();
    out.joints.extend(pose.bones.iter().copied().map(skate3_sim::animation::native_matrix));
}

/// The sim's render pose now, unblended (before a tick has been published).
pub fn joints(skater: &SkaterRuntime, out: &mut Vec<Mat4>) {
    out.clear();
    out.extend(skater.render_pose.iter().copied().map(skate3_sim::animation::native_matrix));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock rig's shape (36 bones): the trajectory root; the hips' tree (1-24); the board's root (25) with its
    /// trucks and wheels (26-31); hands and toes reparented to the board's root (32-35).
    #[test]
    fn the_rider_and_the_board_are_the_rigs_two_bodies() {
        let mut parents: Vec<i32> = vec![-1, 0, 1, 2, 3, 4, 5, 6, 7, 5, 9, 10, 11, 5, 13, 14, 15, 1, 17, 18, 19, 1, 21, 22, 23];
        parents.extend([0, 25, 26, 26, 25, 29, 29, 25, 25, 25, 25]);
        let parents: Vec<Option<u16>> = parents.iter().map(|&p| u16::try_from(p).ok()).collect();
        let groups = groups(&parents, [26, 29, 28, 27, 30, 31], 29);
        assert_eq!(groups[0].id, body::RIDER);
        assert_eq!(groups[0].joints, (1..=24).collect::<Vec<u16>>());
        assert_eq!(groups[1].id, body::BOARD);
        assert_eq!(groups[1].joints, (25..=31).collect::<Vec<u16>>());
        let Cast::Footprint { back, front, ellipses } = &groups[1].cast else { panic!("a footprint") };
        assert_eq!((back.joint, front.joint, ellipses.len()), (29, 26, 3));
    }

    /// The basis is the app's old `render_basis` exactly, to the zeros' signs.
    #[test]
    fn the_basis_is_x_minus_z_y() {
        let old = Mat4::from_cols(Vec4::X, -Vec4::Z, Vec4::Y, Vec4::W);
        assert_eq!(BASIS.to_cols_array().map(f32::to_bits), old.to_cols_array().map(f32::to_bits));
    }
}
