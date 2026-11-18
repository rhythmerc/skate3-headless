//! The stage and park into the sim's collision and grind rails, putting the skater down, and where the rider and the
//! board are. The sim's world is the contract's space (metres, y up) raised to its own floor: the contract's floor
//! (y = 0) is the sim's at [`FLOOR`], its test course's landing floor, where the sim's own ground is. Everything in or
//! out of the sim's world is moved by it here ([`to_sim`], [`from_sim`]).
use engine_core::glam::{Mat4, Vec3};
use engine_core::skate::{SkatePark, SkaterLink};
use engine_core::{Applied, Frame, Stage};
use skate_core::math::Vector3;
use skate_core::physics::board::BodyId;
use skate_core::player::state::PhysicalStateId;
use skate3_sim::physics::ground::PlacedSurfaces;
use skate3_sim::physics::{GamePhysics, SkaterRuntime};

/// How far over the ground point the skater is put down, metres.
pub const PLACE_LIFT: f32 = 0.05;

/// The contract's origin in the sim's world: its floor at the sim's.
pub const FLOOR: Vec3 = Vec3::new(0.0, skate3_sim::physics::ground::FLOOR_HEIGHT, 0.0);

/// A point of the contract's space in the sim's world.
pub fn to_sim(p: Vec3) -> Vec3 {
    p + FLOOR
}

/// A point of the sim's world in the contract's space.
pub fn from_sim(p: Vec3) -> Vec3 {
    p - FLOOR
}

/// A frame (a matrix placing something) of the sim's world in the contract's space.
pub fn frame_from_sim(m: Mat4) -> Mat4 {
    let mut out = m;
    out.w_axis = from_sim(m.w_axis.truncate()).extend(m.w_axis.w);
    out
}

fn v(p: Vec3) -> Vector3 {
    let p = to_sim(p);
    Vector3::new(p.x, p.y, p.z)
}

/// `set_real_surfaces`' arguments: the room's quads, its triangles, its grindable edges, and each piece's surfaces.
pub type Surfaces = (Vec<[Vector3; 4]>, Vec<[Vector3; 3]>, Vec<(Vector3, Vector3)>, Vec<PlacedSurfaces>);

/// The sim's surfaces for `stage` and its `park`: a piece's quads split back into its body and its rail tubes.
pub fn surfaces(stage: &Stage, park: &SkatePark) -> Surfaces {
    let quads = stage.room_quads.iter().map(|q| q.map(v)).collect();
    let triangles = stage.room_triangles.iter().map(|t| t.map(v)).collect();
    let edges = park.room_edges.iter().map(|&(a, b)| (v(a), v(b))).collect();
    let pieces = stage
        .solids
        .iter()
        .map(|solid| {
            let piece = park.piece(solid.id);
            let tubes = piece.map_or(0..0, |p| p.rail_quads.clone());
            let quads = |inside: bool| {
                solid.quads.iter().enumerate().filter(|(i, _)| tubes.contains(i) == inside).map(|(_, q)| q.map(v)).collect()
            };
            PlacedSurfaces {
                quads: quads(false),
                rail_quads: quads(true),
                edges: piece.map_or(Vec::new(), |p| p.edges.iter().map(|&(a, b)| (v(a), v(b))).collect()),
            }
        })
        .collect();
    (quads, triangles, edges, pieces)
}

/// `set_grind_rails`' argument for `park`.
pub fn rails(park: &SkatePark) -> Vec<Vec<Vector3>> {
    park.rails.iter().map(|r| r.iter().map(|&p| v(p)).collect()).collect()
}

/// What the sim has of the stage: the park kept until its stage comes, and the revision taken.
#[derive(Default)]
pub struct StageFeed {
    park: Option<SkatePark>,
    applied: Option<u64>,
}

impl StageFeed {
    /// The park, kept for the stage of its revision ([`StageFeed::set_stage`] takes the two together).
    pub fn set_park(&mut self, park: &SkatePark) -> Applied {
        if self.applied == Some(park.revision) {
            return Applied::Done;
        }
        if self.park.as_ref().is_none_or(|p| p.revision != park.revision) {
            self.park = Some(park.clone());
        }
        Applied::Deferred
    }

    /// The stage (with its park) as the sim's collision and rails: not while the skater grinds (grind state refers to
    /// the rails it has), nor before its park. `Err`: the sim refused it (it isn't offered again).
    pub fn set_stage(&mut self, physics: &mut GamePhysics, skater: &mut SkaterRuntime, stage: &Stage) -> Result<Applied, String> {
        if stage.revision == 0 || self.applied == Some(stage.revision) {
            return Ok(Applied::Done);
        }
        let Some(park) = self.park.as_ref().filter(|p| p.revision == stage.revision) else { return Ok(Applied::Deferred) };
        if skater.is_grinding() {
            return Ok(Applied::Deferred);
        }
        self.applied = Some(stage.revision);
        let (quads, triangles, edges, pieces) = surfaces(stage, park);
        physics.set_real_surfaces(quads, triangles, edges, pieces).map_err(|e| format!("AR collision rebuild failed: {e}"))?;
        physics.set_grind_rails(&rails(park), skater).map_err(|e| format!("AR grind rails rebuild failed: {e}"))?;
        Ok(Applied::Done)
    }
}

/// The teleport matrix for putting the skater down at `point` (the contract's) facing `forward` (its level part):
/// row-major, axes in rows 0-2 (the skater faces row 2), the position in row 3 (the sim's), [`PLACE_LIFT`] over the
/// point.
pub fn place_matrix(point: Vec3, forward: Vec3) -> [[f32; 4]; 4] {
    let forward = forward.with_y(0.0).try_normalize().unwrap_or(Vec3::Z);
    let right = Vec3::Y.cross(forward);
    let p = to_sim(point) + Vec3::Y * PLACE_LIFT;
    [[right.x, right.y, right.z, 0.0], [0.0, 1.0, 0.0, 0.0], [forward.x, forward.y, forward.z, 0.0], [p.x, p.y, p.z, 1.0]]
}

/// Put the skater down on its board at `point`, facing `forward`.
pub fn place_player(skater: &mut SkaterRuntime, point: Vec3, forward: Vec3) -> Result<(), String> {
    skater.travel_to(place_matrix(point, forward))
}

/// Off the board: walking, in the air on foot, climbing back on.
fn off_board(state: PhysicalStateId) -> bool {
    use PhysicalStateId::*;
    matches!(state, BipedGround | BipedAir | OffBoardPushing | LandingOnDeck)
}

/// A row-major native frame (axes in rows 0-2, the position in row 3) as a [`Frame`]: forward its row 2, up row 1.
fn frame(m: &[[f32; 4]; 4]) -> Frame {
    let row = |i: usize| Vec3::new(m[i][0], m[i][1], m[i][2]);
    Frame { position: from_sim(row(3)), forward: row(2).normalize_or(Vec3::Z), up: row(1).normalize_or(Vec3::Y) }
}

/// Where the skater is: the board from its deck's body; the rider from the animation's root, the frame the sim's
/// biped and camera take as the skater's own (`animation_to_world`, its stance axes turned as the sim's effective root
/// turns them). On the board the sim builds the root from the board (`SkeletonRootFrames::update`); off it, from the
/// biped's own world frame (the off-board ground and air skeletons reset it), so it walks away from the board.
pub fn link(physics: &GamePhysics, skater: &SkaterRuntime) -> SkaterLink {
    let at = skate3_sim::Link::of(physics, skater);
    let deck = physics.board.part_transforms()[BodyId::Deck.index()];
    let up = Vec3::from_array(deck.basis.columns[1]);
    let mut root = skater.animated_skeleton.roots.animation_to_world;
    // (the sim's GetEffectiveRoot: a goofy stance's right and forward turned round)
    if skater.player_input.processed.flags_2476 & 4 != 0 {
        for axis in [0, 2] {
            root[axis] = root[axis].map(|v| -v);
        }
    }
    SkaterLink {
        rider: Some(frame(&root)),
        board: Some(Frame { position: from_sim(at.deck), forward: at.deck_forward, up }),
        on_board: !off_board(skater.player_state.current()),
        grinding: at.grinding,
        size: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::Solid;
    use engine_core::skate::ParkPiece;

    #[test]
    fn a_place_is_level_and_lifted() {
        let m = place_matrix(Vec3::ZERO, Vec3::new(0.0, -0.5, 1.0));
        assert_eq!(m[0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(m[2], [0.0, 0.0, 1.0, 0.0]);
        assert!((m[3][1] - FLOOR.y - PLACE_LIFT).abs() < 1e-6, "on the sim's floor");
    }

    /// The room's floor is the sim's: a point goes in raised to it and comes out lowered; a frame keeps its axes.
    #[test]
    fn the_rooms_floor_is_the_sims() {
        assert!(FLOOR.y < -1.0 && FLOOR.x == 0.0 && FLOOR.z == 0.0);
        let p = Vec3::new(1.0, 0.0, -2.0);
        assert_eq!(to_sim(p), Vec3::new(1.0, FLOOR.y, -2.0));
        assert_eq!(from_sim(to_sim(p)), p);
        let m = Mat4::from_rotation_y(0.7) * Mat4::from_translation(Vec3::new(0.0, FLOOR.y + 0.5, 3.0));
        let out = frame_from_sim(m);
        assert_eq!((out.x_axis, out.y_axis, out.z_axis), (m.x_axis, m.y_axis, m.z_axis));
        assert!((out.w_axis.truncate() - (m.w_axis.truncate() - FLOOR)).length() < 1e-6 && out.w_axis.w == 1.0);
    }

    #[test]
    fn a_pieces_quads_split_back_into_body_and_tubes() {
        let q = |y: f32| [Vec3::new(0.0, y, 0.0), Vec3::X, Vec3::Z, Vec3::ONE];
        let stage = Stage {
            revision: 3,
            solids: vec![Solid { id: 9, quads: vec![q(1.0), q(2.0), q(3.0)] }, Solid { id: 4, quads: vec![q(4.0)] }],
            ..Default::default()
        };
        let park = SkatePark {
            revision: 3,
            pieces: vec![ParkPiece { solid: 9, rail_quads: 1..3, edges: vec![(Vec3::ZERO, Vec3::X)] }],
            ..Default::default()
        };
        let (_, _, _, pieces) = surfaces(&stage, &park);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].quads.iter().map(|q| q[0].y).collect::<Vec<_>>(), [1.0 + FLOOR.y]);
        assert_eq!(pieces[0].rail_quads.iter().map(|q| q[0].y).collect::<Vec<_>>(), [2.0 + FLOOR.y, 3.0 + FLOOR.y]);
        assert_eq!(pieces[0].edges.len(), 1);
        assert_eq!((pieces[1].quads.len(), pieces[1].rail_quads.len(), pieces[1].edges.len()), (1, 0, 0));
    }

    #[test]
    fn a_park_waits_for_its_stage_and_a_taken_one_is_done() {
        let mut feed = StageFeed::default();
        let park = SkatePark { revision: 2, ..Default::default() };
        assert_eq!(feed.set_park(&park), Applied::Deferred);
        assert_eq!(feed.park.as_ref().map(|p| p.revision), Some(2));
        feed.applied = Some(2);
        assert_eq!(feed.set_park(&park), Applied::Done);
    }
}
