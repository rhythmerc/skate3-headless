//! The façade: load the sim from an imported bundle, step it in the same stages the host's fixed
//! tick runs, and read its outputs as plain data (docs/skate3-sim-split.md section 2.2).
//!
//! Two ways to drive it, sharing one implementation:
//! - own a [`Sim`] and call [`Sim::tick`] (or its stages) once per fixed step, resetting the
//!   host's fixed timestep from [`Sim::period`] after every tick (the sim sets its own cadence:
//!   camera slow-mo changes the clock);
//! - keep the parts as separate host resources ([`Sim::into_parts`]) and call the stage
//!   functions the methods are made of ([`crate::input::publish`],
//!   [`crate::physics::controls::sample`], [`crate::physics::advance`],
//!   [`crate::physics::audio_observation::observe`]) from the host's own schedule, which is what
//!   skate-game does while its other systems still read the parts directly.
//!
//! Host hooks between the stages (skate-game's session marker between input and controls, its
//! multiplayer proxies between controls and physics) are why the stages are public.
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use skate_core::{
    animation::output::NativeMatrix, camera::CameraFrame, input::tick::TickInput,
    physics::board::BodyId, physics::drive_frames::RetailAffineTransform,
};
use skate_data::{GameAssets, skate_map::SkateMap};
use tracing::info_span;

use crate::{
    camera::CameraRuntime,
    difficulty::Difficulty,
    graph_runtime::StockGraphs,
    input::ControllerInput,
    physics::{AudioObservationCursor, GamePhysics, PlayerControls, SkaterRuntime},
    skate_audio::PlayerAudioObservation,
};

/// The player's imported Skate 3 data: where it is and its asset manifest.
pub struct Bundle {
    root: PathBuf,
    manifest: GameAssets,
}

impl Bundle {
    /// Read the stock gameplay configuration and the asset manifest under `root`. `stage` hears
    /// each step's name before it runs (skate-game prints them as `REPORT_META stage=...`).
    pub fn open(root: &Path, stage: &mut dyn FnMut(&str)) -> Result<Self, String> {
        stage("gameplay_configuration");
        skate_data::input_config::StockGameplayConfig::load(root).map_err(|e| e.to_string())?;
        stage("asset_manifest");
        let manifest = info_span!("load_manifest")
            .in_scope(|| GameAssets::load(root))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.to_owned(),
            manifest,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The character scene and other assets the host's renderer loads itself.
    pub fn manifest(&self) -> &GameAssets {
        &self.manifest
    }

    pub fn into_manifest(self) -> GameAssets {
        self.manifest
    }
}

/// What to load: the difficulty, the world (a `.skate` map, or the built-in course) and how far
/// along x to move the spawn (a second local player).
pub struct SimConfig<'a> {
    pub difficulty: Difficulty,
    pub map: Option<&'a SkateMap>,
    pub spawn_offset: f32,
}

/// The whole Skate 3 simulation for one local skater.
pub struct Sim {
    pub physics: GamePhysics,
    pub skater: SkaterRuntime,
    pub controls: PlayerControls,
    pub graphs: StockGraphs,
    /// The stock gameplay camera. Optional because a host that draws with its own view can skip
    /// loading it, but ticking needs it: off-board steering and the camera tricks read its frame.
    pub camera: Option<CameraRuntime>,
    pub input: ControllerInput,
    /// The actions [`Sim::publish_input`] published for this tick.
    pub published: TickInput,
    audio: AudioObservationCursor,
    view: Option<[[f32; 3]; 3]>,
}

/// [`Sim`] taken apart, for a host that keeps each part as its own resource.
pub struct Parts {
    pub physics: GamePhysics,
    pub skater: SkaterRuntime,
    pub controls: PlayerControls,
    pub graphs: StockGraphs,
    pub camera: Option<CameraRuntime>,
}

/// The skater's pose for drawing: native model-space bone matrices and the animation root.
pub struct SkaterPose<'a> {
    /// Changes whenever a new pose is published (once per completed tick).
    pub generation: u64,
    pub root: Mat4,
    pub bones: &'a [NativeMatrix],
}

impl<'a> SkaterPose<'a> {
    /// [`Sim::pose`], for a host keeping the parts ([`Sim::into_parts`]).
    pub fn of(skater: &'a SkaterRuntime) -> Self {
        SkaterPose {
            generation: skater.pose_generation,
            root: crate::animation::native_matrix(skater.animated_skeleton.roots.animation_to_world),
            bones: &skater.render_pose,
        }
    }
}

/// Where the board is, for a host placing things relative to the skater.
#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub deck: Vec3,
    pub deck_forward: Vec3,
    pub grinding: bool,
}

impl Link {
    /// [`Sim::link`], for a host keeping the parts ([`Sim::into_parts`]).
    pub fn of(physics: &GamePhysics, skater: &SkaterRuntime) -> Self {
        let deck = physics.board.part_transforms()[BodyId::Deck.index()];
        let t = deck.translation;
        let f = deck.basis.columns[2];
        Link {
            deck: Vec3::new(t.x, t.y, t.z),
            deck_forward: Vec3::new(f[0], f[1], f[2]),
            grinding: skater.is_grinding(),
        }
    }
}

impl Sim {
    /// Load everything a tick needs except the camera (see [`Sim::load_camera`]). `Send`, so it
    /// can run on a worker thread while the host keeps its own loop serviced.
    pub fn load(
        bundle: &Bundle,
        config: &SimConfig,
        stage: &mut dyn FnMut(&str),
    ) -> Result<Self, String> {
        let root = bundle.root();
        stage("stock_graphs");
        let graphs =
            info_span!("load_graphs").in_scope(|| StockGraphs::load(root, bundle.manifest()))?;
        stage("map_validation");
        if let Some(map) = config.map {
            crate::skate_world::validate_runtime(map)?;
        }
        eprintln!(
            "SKATE_DIFFICULTY mode={} native_index={}",
            config.difficulty.key(),
            config.difficulty as u32
        );
        stage("physics_initialization");
        let mut physics = info_span!("load_physics").in_scope(|| {
            GamePhysics::load_with_difficulty(root, config.map, config.difficulty)
        })?;
        if config.spawn_offset != 0.0 {
            let mut spawn = physics.board.part_transforms()[BodyId::Deck.index()];
            spawn.translation.x += config.spawn_offset;
            physics.board.set_transform(spawn);
        }
        stage("skater_initialization");
        let skater = info_span!("load_skater").in_scope(|| {
            SkaterRuntime::load(root, &graphs, &physics, config.difficulty.key())
        })?;
        stage("controls_initialization");
        let controls = PlayerControls::load(root)?;
        Ok(Self {
            physics,
            skater,
            controls,
            graphs,
            camera: None,
            input: ControllerInput::default(),
            published: crate::input::idle_tick_input(),
            audio: AudioObservationCursor::default(),
            view: None,
        })
    }

    /// Load the stock gameplay camera ([`Sim::tick`] needs it).
    pub fn load_camera(&mut self, root: &Path) -> Result<(), String> {
        self.camera = Some(CameraRuntime::load(root)?);
        Ok(())
    }

    pub fn into_parts(self) -> Parts {
        Parts {
            physics: self.physics,
            skater: self.skater,
            controls: self.controls,
            graphs: self.graphs,
            camera: self.camera,
        }
    }

    // ---- inputs ----

    /// The view the off-board stick is relative to when it isn't the game camera's: the native
    /// presentation basis (right, up, at) of the player's own eyes in the headset.
    pub fn set_view(&mut self, view: Option<[[f32; 3]; 3]>) {
        self.view = view;
    }

    // ---- stepping ----

    /// The fixed step the sim wants now. It can change after any tick (camera slow-mo), so a
    /// host reapplies it to its fixed timestep after every [`Sim::advance`].
    pub fn period(&self) -> std::time::Duration {
        self.physics.period()
    }

    /// Stage 1: publish this tick's actions from the pads collected into [`Sim::input`].
    /// `gameplay` false (a menu is open) discards gameplay presses first.
    pub fn publish_input(&mut self, gameplay: bool) {
        self.published = crate::input::publish(&mut self.input, gameplay);
    }

    /// Stage 2: sample the controllers on the published actions.
    pub fn sample_controls(&mut self) -> Result<(), String> {
        let camera = self.camera.as_ref().ok_or("the sim's camera is not loaded")?;
        crate::physics::controls::sample(
            &self.published,
            &self.physics,
            &self.skater,
            camera,
            self.view,
            &mut self.controls,
        )
    }

    /// Stage 3: one physics/animation tick, then its player-audio observation (once per tick).
    /// After an error the sim stays failed and further ticks are refused.
    pub fn advance(&mut self) -> Result<Option<PlayerAudioObservation>, String> {
        if self.physics.failed {
            return Err("the simulation has failed".into());
        }
        let camera = self.camera.as_mut().ok_or("the sim's camera is not loaded")?;
        crate::physics::advance(
            &mut self.physics,
            &mut self.skater,
            &mut self.controls,
            &self.graphs,
            &self.published,
            camera,
        )?;
        Ok(crate::physics::audio_observation::observe(
            &self.physics,
            &self.skater,
            camera,
            &mut self.audio,
        ))
    }

    /// All three stages, for a host with nothing to run between them.
    pub fn tick(&mut self, gameplay: bool) -> Result<Option<PlayerAudioObservation>, String> {
        self.publish_input(gameplay);
        self.sample_controls()?;
        self.advance()
    }

    // ---- outputs ----

    pub fn pose(&self) -> SkaterPose<'_> {
        SkaterPose::of(&self.skater)
    }

    /// Deck, trucks and wheels, in [`BodyId`] order.
    pub fn board(&self) -> [RetailAffineTransform; 7] {
        self.physics.board.part_transforms()
    }

    pub fn link(&self) -> Link {
        Link::of(&self.physics, &self.skater)
    }

    /// The stock gameplay camera's latest frame (a desktop host draws with it).
    pub fn camera_frame(&self) -> Option<&CameraFrame> {
        self.camera.as_ref().and_then(|c| c.frame.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sim_can_load_on_a_worker() {
        fn send<T: Send>() {}
        send::<Sim>();
        send::<Bundle>();
    }

    #[test]
    fn a_missing_bundle_is_refused_before_any_stage_but_the_first() {
        let mut stages = Vec::new();
        let result = Bundle::open(Path::new("/nonexistent/skate3"), &mut |s| {
            stages.push(s.to_owned())
        });
        assert!(result.is_err());
        assert_eq!(stages, ["gameplay_configuration"]);
    }

    #[test]
    #[ignore = "requires SKATE3_ASSET_ROOT pointing to the converted Skate 3 data"]
    fn the_facade_ticks_and_publishes_poses_and_audio() {
        let root = std::env::var_os("SKATE3_ASSET_ROOT").expect("set SKATE3_ASSET_ROOT");
        let root = Path::new(&root);
        let bundle = Bundle::open(root, &mut |_| {}).unwrap();
        let config = SimConfig {
            difficulty: Difficulty::Easy,
            map: None,
            spawn_offset: 0.0,
        };
        let mut sim = Sim::load(&bundle, &config, &mut |_| {}).unwrap();
        sim.load_camera(root).unwrap();
        let mut observations = 0;
        for _ in 0..120 {
            observations += usize::from(sim.tick(true).unwrap().is_some());
            assert!(sim.period() > std::time::Duration::ZERO);
        }
        assert_eq!(observations, 120);
        let pose = sim.pose();
        assert!(pose.generation > 0);
        assert!(!pose.bones.is_empty());
        assert!(sim.link().deck.is_finite());
    }
}
