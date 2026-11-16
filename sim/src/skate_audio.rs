//! What a completed physics tick tells the player-audio graph, and the graph itself
//! ([`player`]). Playing the mixed PCM is the host's (skate-game's `skate_audio`).
use skate_core::physics::phase::PhysicsEvent;

#[path = "player_audio.rs"]
pub mod player;


/// One completed local-player physics tick, ready for the player-audio runtime.
///
/// This is transport only: it preserves the physics event order and raw surface vote without
/// claiming that a physics material id is an authored audio-material id. Patch-message creation,
/// random selection, and archive lookup remain the audio runtime's responsibilities.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerAudioObservation {
    pub tick: u64,
    pub state: u32,
    pub board_speed: f32,
    pub rider_speed: f32,
    pub grounded: bool,
    /// The authoritative physical state is a grind. This is distinct from a deck scrape or a
    /// wheel contact, which can occur without an admitted grind.
    pub grinding: bool,
    /// Retained native Grind PhysOut fields. These are the inputs consumed by the retail grind
    /// audio updater; a ground-wheel material is not a substitute for its authored surface id.
    pub grind_family: u32,
    pub grind_substate: u32,
    pub grind_audio_surface: u32,
    pub grind_impact_speed: f32,
    pub wiping_out: bool,
    pub landed: bool,
    /// Downward speed immediately before a landing contact, in metres per second.
    pub landing_impact_speed: f32,
    pub landing_clean: bool,
    pub landing_sketchy: bool,
    pub landing_type: u32,
    pub landing_spin: f32,
    pub landing_sideways_speed: f32,
    pub footstep_strength: f32,
    pub footstep_bone: i32,
    pub foot_push_speed: f32,
    /// Per-foot ground support while walking (left, right), from the offboard foot manager.
    pub feet_supported: [bool; 2],
    /// Packed foot-query audio material (low seven bits), independent of the board wheels.
    pub foot_surface: u32,
    pub contact_count: u32,
    /// The stock MotionGraph's powerslide flag. This is the retail trigger for `Class_Squeaks`;
    /// ordinary speed loss must not be treated as a powerslide.
    pub powersliding: bool,
    /// The authored scoring descriptor active on this tick, resolved through the retail scoring
    /// catalog. The identifier is retained because its family (ollie, kickflip, shuv, and so on)
    /// is one of the inputs used by the trick-foley selectors.
    pub trick_id: Option<usize>,
    pub trick_identifier: Option<String>,
    pub animation_name: Option<String>,
    pub riding_switch: bool,
    pub riding_fakie: bool,
    pub nollie: bool,
    /// The retail audio-material vote from the low seven bits of the four wheel-query tags.
    /// This is intentionally not the friction/physics surface stored in bits 7..11.
    pub wheel_surface: u32,
    pub events: Vec<PhysicsEvent>,
    /// Front-end one-shot ids to play this frame, retail's `GlobalFEPlaySound` (`sub_825DFAF0`)
    /// arguments. `session_marker` emits these and the forwarder attaches them here; they are not
    /// physics events, so they do not belong in `events`.
    pub frontend_sounds: Vec<u64>,
    /// The native PhysOut fields the retail audio-state bridge `sub_824B0DA8` reads.
    pub retail: RetailAudioInputs,
}

/// Raw native PhysOut fields for the retail audio-state bridge `sub_824B0DA8`, which copies them
/// from the per-skater record `sub_827A1B78` builds, and for the PhysOut audio conditioner
/// (`sub_82772748`, PhysOut bundle slot B+40) whose output that record also packs. Transport
/// only: the player-audio worker runs the conditioner, the record packing and the bridge the way
/// retail does (`player_audio/audio_state.rs`). Field docs name the native PhysOut slot and
/// offset (B = `[[player+1808]]->vfunc92()`).
#[derive(Clone, Debug, PartialEq)]
pub struct RetailAudioInputs {
    /// The physics step (Processed+2604). The bridge's `f1` (hold clock +312) is the audio frame
    /// time; the engine runs the bridge once per physics observation.
    pub dt: f32,
    /// SkateboardMotion+164, the board's ground speed in m/s (audio state +208).
    pub ground_speed: f32,
    /// SystemReckoning+16, the COM velocity (audio state +96; its length is +212; its Y is the
    /// conditioner's landing sample, `sub_82772B88`).
    pub com_velocity: [f32; 3],
    /// SystemReckoning+64, the skater position (audio state +48).
    pub position: [f32; 3],
    /// Collision+0 wheel count (audio state +200 = `(R152 >> 20) & 7`).
    pub wheel_count: u32,
    /// KnownAir+176 time in state (audio state +236).
    pub air_time_in_state: f32,
    /// Air+184 time until landing (audio state +240).
    pub air_time_until_landing: f32,
    /// KnownAir+200 jump height (audio state +260).
    pub air_jump_height: f32,
    /// Player state bytes 52..87 as published by the state output (index = offset - 52).
    pub state_flags: [bool; 36],
    /// OffBoard bytes 306 (left) and 307 (right).
    pub offboard_feet: [bool; 2],
    /// Air FootPlantManager bytes 449 (left) and 450 (right).
    pub footplant: [bool; 2],
    /// Interaction+0 `AudibleFootStepStrength` (audio state +796).
    pub footstep_strength: f32,

    /// State+12 physical category (`physical.state.category_12`).
    pub state_category: u32,
    /// State+16 physical state id (`physical.state.state_16`).
    pub state: u32,
    /// Filtered+0 (`[B+64]+0`, `physical.filtered_state_0`); 7 = OffboardAir.
    pub filtered_state: u32,
    /// Air byte438, written by ProcessOutput `sub_82DB6EC0` (82DB7620..764C): `200 <= State+16 < 300 &&
    /// Collision+0 == 0`. The board FillPhysOut `82C02A80` has already run in that
    /// ProcessOutput, so this is the current tick's wheel count.
    pub in_known_air: bool,
    /// Grinds byte316 (`grinds.grinding_316`).
    pub grinding: bool,
    /// Grinds+136 physical grind family (`grinds.words_136_140[0]`).
    pub grind_family: u32,
    /// Grinds+216 grind audio material (`grinds.audio_surface_216`).
    pub grind_audio_surface: u32,
    /// Grinds+128 grind impact speed (`grinds.impact_speed_128`).
    pub grind_impact_speed: f32,
    /// Grinds byte323 (`grinds.flag_323`).
    pub grind_flag_323: bool,
    /// Air byte440 = Processed2468 bit 22 (ProcessOutput `sub_82DB6EC0`).
    pub air_440: bool,
    /// Air+112 jump velocity delta (`air.jump_velocity_delta_112`).
    pub jump_velocity_delta: [f32; 3],
    /// Air byte448 (`air.flag_448`): the foot materials come from Air+224 instead of OffBoard.
    pub footplant_448: bool,
    /// Air+224 footplant surface tag (`air.footplant_surface_224`).
    pub footplant_surface: u32,
    /// OffBoard+52 and +56. ProcessOutput `sub_82DB6EC0` stores Processed+2596 into both.
    pub offboard_surface: u32,
    /// OffBoard bytes 309 / 310: ProcessOutput `sub_82DB6EC0` stores Processed2480 bits 18
    /// and 8.
    pub offboard_309: bool,
    pub offboard_310: bool,
    /// OffBoard byte311, the board possession owner's held flag (`off_board.flag_311`).
    pub offboard_311: bool,
    /// Skeleton byte599 end of bail (`skeleton.over_599`).
    pub skeleton_599: bool,
    /// Skeleton bytes 600/601, feet inside the deck box (`82BF22A0`).
    pub feet_in_deck_box: [bool; 2],
    /// Skeleton+192 / +208: local toe velocities (`82BF22A0`, `foot_physical.output`).
    pub foot_local_velocity: [[f32; 3]; 2],
    /// Skeleton+224 / +240: world foot velocities (`82BF22A0`).
    pub foot_world_velocity: [[f32; 3]; 2],
    /// Collision bytes 3296..3299: per-wheel contact (`82C02A80` from CollisionInfo+844).
    pub wheel_contacts: [bool; 4],
    /// Collision bytes 3473 / 3474: front / back truck contact (CollisionInfo+848/+849).
    pub truck_contacts: [bool; 2],
    /// Collision byte3475: deck contact (CollisionInfo+850).
    pub deck_contact: bool,
    /// Collision+3376+16i: the wheel's contact normal, zero when the wheel is not in contact
    /// (`82C02A80`, loc_82C02D9C..2DA8 for wheel 0).
    pub wheel_contact_normals: [[f32; 3]; 4],
    /// Collision+3440+4i: wheel audio material, `tag & 0x7F` (0 = no hit).
    pub wheel_audio_surfaces: [u32; 4],
    /// Collision+3456+4i: wheel seam pattern, `(tag >> 12) & 0xF`.
    pub wheel_seam_patterns: [u32; 4],
    /// Collision+4/+8/+12: front truck / back truck / deck audio material (0 = no report).
    pub part_audio_surfaces: [u32; 3],
    /// Collision+20: deck slide speed (`82C02A80` 82C034E8..3570).
    pub deck_slide_speed: f32,
    /// Collision+24: deck scrape (`82C02A80` 82C03478..34D8).
    pub deck_scrape: f32,
    /// SkateboardMotion+64 / +80: deck angular / linear velocity.
    pub angular_velocity: [f32; 3],
    pub linear_velocity: [f32; 3],
    /// SkateboardMotion+184 = SkateboardBody+256, the filtered signed deck tilt.
    pub deck_tilt: f32,
    /// SkateboardMotion+200 = Processed+2764.
    pub motion_200: f32,
    /// SkateboardMotion+208+16i: wheel body linear velocities.
    pub wheel_velocities: [[f32; 3]; 4],
    /// PhysOut slot B+0, +0/+16/+32: rows of the deck part transform (`82585CB0`, part 6).
    pub deck_rows: [[f32; 3]; 3],
    /// PhysOut slot B+0, +80: row 1 of the effective deck transform (`82C01BF8`).
    pub effective_deck_up: [f32; 3],
    /// PhysOut slot B+0, +208+16i: wheel body positions (audio state +384+16i).
    pub wheel_positions: [[f32; 3]; 4],
    /// The camera the listener `*(0x830CFDD4)` follows (`sub_8248CC08`): its world matrix row 2
    /// (native At, the view direction) and row 3 (position). `None` before the first camera frame.
    pub camera: Option<([f32; 3], [f32; 3])>,
    /// The deck position and effective forward axis, standing in for the board PhysOut record's
    /// `+144` / `+128` that the second position controller (`60010020`) is bound to. UNVERIFIED:
    /// the writer of B+0 +128/+144 was not traced.
    pub deck_position: [f32; 3],
    pub deck_forward: [f32; 3],
    /// The score module's combo multiplier (`sub_82DA4238` stores it at the score output +56;
    /// `sub_827A2E88` turns it into the frame record's x1.5/x2/x3 tier flags).
    pub combo_multiplier: f32,
    /// Gameplay is paused (the pause menu). Retail keeps the bridge, the components and the voices
    /// running and ducks the mix through SFXObj_Pause (`sub_824E1D00`) input 0 instead.
    pub paused: bool,
    /// Ground+80: the wheel-contact normal (`ground.vector_80`).
    pub ground_normal: [f32; 3],
    /// Ground+264 = Processed+2676, the turn attribute.
    pub turn: f32,
    /// Ground+300 = Processed+2624, the jump strength attribute.
    pub jump_strength: f32,
    /// PhysOutScoring2 (B+60)+152: the EScorableID `sub_82DAC498` looks up (`sub_82DA5AC8`)
    /// while the score packet carries flag bit 24 or 25, else -1.
    pub scorable_id: i32,
    /// Skeleton+288 / +304 / +320: body+48 (angular velocity) of ragdoll parts 23 / 20 / 16,
    /// copied by Skeleton Fill `sub_82BE1AE8` from `[[[SkeletonState+6448]+24]+8]` +2284 /
    /// +1996 / +1612 (part record +76 → body).
    pub ragdoll_spin: [[f32; 3]; 3],
    /// Skeleton+560 / +564 / +568 / +572 (`sub_82BE1AE8`): |physical-record velocity of parts
    /// 17 / 21 / 4 / 8 (SkeletonState+10368 / +10432 / +10160 / +10224) − the COM velocity
    /// (SkeletonState+16176 = Reckoning+16)|.
    pub limb_speeds: [f32; 4],
    /// Skeleton+144 / +160 (`sub_82BE1AE8`): translations of physical parts 15 / 19
    /// (SkeletonState+9024 / +9280, the physical pose at +8016).
    pub toe_positions: [[f32; 3]; 2],
    /// Collision+80..+195, the ragdoll contact publication `sub_82BD60C8`.
    pub body_contacts: BodyContacts,
    /// Skeleton+516 = Pumping+56 absorption (−speed × angular speed), stored by ProcessOutput
    /// `sub_82DB6EC0` from `[player+1820]+56` right after Skeleton Fill (audio state +712).
    pub pump_absorption: f32,
}

/// Collision+80..+195 as the ragdoll contact publisher `sub_82BD60C8` writes them from the
/// SkeletonCollision object (`[SkeletonState+6448]+160`, updated by `sub_82BD4A30`). Slot i is
/// contact region i (table `0x820CFCB0`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyContacts {
    /// Region part ≠ −1 (SkeletonCollision+1200+4i).
    pub contact: [bool; 8],
    /// |physical-record velocity change of the part (record+4048+16p) · region normal
    /// (SkeletonCollision+1008+16i)| × part weight (record+4560+4p), 0 without a contact. The
    /// worker finishes Collision+80+4i = clamp(max(this × K164, 0.001), 0, 1) with the vault's
    /// physics_collision layout +164 (`sub_82BD60C8` 82BD68F0 loop).
    pub weighted_change: [f32; 8],
    /// Collision+112+4i: the region's tangential speed (SkeletonCollision+944+4i), 0 without.
    pub slide: [f32; 8],
    /// Collision+144+4i: the region's material, `tag & 0x7F` (SkeletonCollision+976+4i),
    /// written only with a contact; the PhysOut reset leaves 0 otherwise.
    pub material: [u32; 8],
    /// Collision bytes 176 / 177: the groin (part 23) / face (part 1) specific contact is current
    /// this frame (SkeletonCollision bytes 4008 / 4009).
    pub specific_current: [bool; 2],
    /// Collision+180 / +184 / +188 / +192: SkeletonCollision+4056 (maximum group-8 force),
    /// +4048 (maximum skater force), +4052 (other skater, −1 none), +4060 (maximum group-11
    /// force).
    pub group_8_force: f32,
    pub skater_force: f32,
    pub other_skater: i32,
    pub group_11_force: f32,
}

impl Default for RetailAudioInputs {
    fn default() -> Self {
        Self {
            camera: None,
            deck_position: [0.0; 3],
            deck_forward: [0.0; 3],
            combo_multiplier: 1.0,
            paused: false,
            dt: 0.0,
            ground_speed: 0.0,
            com_velocity: [0.0; 3],
            position: [0.0; 3],
            wheel_count: 0,
            air_time_in_state: 0.0,
            air_time_until_landing: 0.0,
            air_jump_height: 0.0,
            state_flags: [false; 36],
            offboard_feet: [false; 2],
            footplant: [false; 2],
            footstep_strength: 0.0,
            state_category: 0,
            state: 0,
            filtered_state: 0,
            in_known_air: false,
            grinding: false,
            grind_family: u32::MAX,
            grind_audio_surface: 0,
            grind_impact_speed: 0.0,
            grind_flag_323: false,
            air_440: false,
            jump_velocity_delta: [0.0; 3],
            footplant_448: false,
            footplant_surface: 0,
            offboard_surface: 0,
            offboard_309: false,
            offboard_310: false,
            offboard_311: false,
            skeleton_599: false,
            feet_in_deck_box: [false; 2],
            foot_local_velocity: [[0.0; 3]; 2],
            foot_world_velocity: [[0.0; 3]; 2],
            wheel_contacts: [false; 4],
            truck_contacts: [false; 2],
            deck_contact: false,
            wheel_contact_normals: [[0.0; 3]; 4],
            wheel_audio_surfaces: [0; 4],
            wheel_seam_patterns: [0; 4],
            part_audio_surfaces: [0; 3],
            deck_slide_speed: 0.0,
            deck_scrape: 0.0,
            angular_velocity: [0.0; 3],
            linear_velocity: [0.0; 3],
            deck_tilt: 0.0,
            motion_200: 0.0,
            wheel_velocities: [[0.0; 3]; 4],
            deck_rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            effective_deck_up: [0.0, 1.0, 0.0],
            wheel_positions: [[0.0; 3]; 4],
            ground_normal: [0.0, 1.0, 0.0],
            turn: 0.0,
            jump_strength: 0.0,
            scorable_id: -1,
            ragdoll_spin: [[0.0; 3]; 3],
            limb_speeds: [0.0; 4],
            toe_positions: [[0.0; 3]; 2],
            body_contacts: BodyContacts {
                other_skater: -1,
                ..BodyContacts::default()
            },
            pump_absorption: 0.0,
        }
    }
}

