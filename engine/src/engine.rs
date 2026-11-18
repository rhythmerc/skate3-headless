//! Skate 3 as the runtime drives it ([`engine_core::EngineCore`]): the sim (`skate3_sim::Sim`) loaded on a worker from
//! the player's imported data ([`factory`]), its pad made from the Touch controllers, given the stage and park as its
//! collision and rails, ticked, and read after each tick: the skater's pose, where it is, its trick, its sound (its own
//! graph's stream, or its ticks as the skating sounds the runtime carries) and its trick display.
//!
//! What the app did around the sim's tick is here too: the skater's gameplay preferences every frame (the default
//! profile's), the off-board stick relative to the player's eyes, the score multiplier's sounds each tick.
use crate::sound::{Heard, StreamFeed};
use crate::{hud, input, model, sound, stage, trick};
use engine_core::audio::StreamFactory;
use engine_core::glam::Vec3;
use engine_core::hud::{HudFrame, Rgba8, TextureId};
use engine_core::skate::sound::SkateSounds;
use engine_core::skate::{SkateMode, SkatePark, SkaterLink, TrickText};
use engine_core::{Applied, EngineCore, EngineFactory, Focus, Model, PadHeld, PlacePlayer, Pose, Stage, TouchState};
use skate3_sim::difficulty::Difficulty;
use skate3_sim::input::device::DeviceError;
use std::path::PathBuf;
use std::time::Duration;

/// Its one model, the skater.
pub const SKATER: engine_core::ModelId = 0;

/// Its settings' keys ([`EngineCore::set_setting`]).
pub mod settings {
    /// The trick display's multiplier circle always shown (1, the default), or fading in and out with the line as
    /// retail has it (0): `crate::hud::Hud::set_circle_always`.
    pub const CIRCLE_ALWAYS: &str = "hud_circle_always";
}

/// Skate 3 made on the runtime's worker from the player's imported data (`assets`: the converted installation's
/// `assets` folder). `own_sound`: its own sound graph (the decoded sound banks are there), else the skating sounds the
/// runtime carries.
pub fn factory(assets: PathBuf, difficulty: Difficulty, own_sound: bool) -> EngineFactory {
    Box::new(move || Ok(Box::new(load(assets, difficulty, own_sound)?) as Box<dyn EngineCore>))
}

/// [`factory`]'s work: the sim and the rest loaded.
fn load(assets: PathBuf, difficulty: Difficulty, own_sound: bool) -> Result<Skate3, String> {
    {
        let started = std::time::Instant::now();
        let mut stage = |name: &str| eprintln!("REPORT_META stage={name}");
        let bundle = skate3_sim::Bundle::open(&assets, &mut stage)?;
        let config = skate3_sim::SimConfig { difficulty, map: None, spawn_offset: 0.0 };
        let mut sim = skate3_sim::Sim::load(&bundle, &config, &mut stage)?;
        sim.load_camera(&assets)?;
        let model = model::model(&bundle.manifest().character_scene, &sim.skater);
        // (its trick display: without the HUD files, none, and the runtime's corner readout shows the tricks)
        let root = hud::root(&assets);
        let hud = match hud::Hud::load(&root, sim.skater.scoring.hud_input()) {
            Ok(mut hud) => {
                hud.set_circle_always(true);
                Some(hud)
            }
            Err(e) => {
                eprintln!("Skate 3: its scoring HUD could not load from {}: {e}", root.display());
                None
            }
        };
        let sound = if own_sound {
            let (feed, stream) = sound::stream(assets.clone());
            Sound::Own(feed, Some(stream))
        } else {
            Sound::Carried(Heard::default())
        };
        eprintln!("Skate 3 ready in {:.0} ms", started.elapsed().as_secs_f32() * 1000.0);
        Ok(Skate3::new(sim, model, hud, sound))
    }
}

enum Sound {
    /// Its own graph: the ticks' feed, and the stream until the runtime takes it.
    Own(StreamFeed, Option<StreamFactory>),
    /// The runtime's skating sounds, from its ticks.
    Carried(Heard),
}

/// The running sim.
pub struct Skate3 {
    sim: skate3_sim::Sim,
    stage: stage::StageFeed,
    pad: input::ExternalPad,
    model: Option<Model>,
    hud: Option<hud::Hud>,
    sound: Sound,
    pose: Pose,
    /// The last tick left a pose.
    posed: bool,
}

impl Skate3 {
    fn new(sim: skate3_sim::Sim, model: Model, hud: Option<hud::Hud>, sound: Sound) -> Self {
        Self {
            sim,
            stage: stage::StageFeed::default(),
            pad: input::ExternalPad::default(),
            model: Some(model),
            hud,
            sound,
            pose: Pose::default(),
            posed: false,
        }
    }
}

/// The skater's gameplay preferences, every frame: the default profile's (trucks and wheels 0.7, stance 1, style and
/// posture 0, the stock gestures). Choosing them comes back with character customisation, through the contract.
fn preferences(sim: &mut skate3_sim::Sim) {
    static DEFAULT: std::sync::LazyLock<serde_json::Value> =
        std::sync::LazyLock::new(|| serde_json::json!({"selections": {}, "morphs": {}}));
    skate3_sim::physics::apply_preferences(&DEFAULT, &mut sim.physics, &mut sim.skater.animation);
}

impl EngineCore for Skate3 {
    fn period(&self) -> Duration {
        self.sim.period()
    }

    /// The pad in controller slot 0 (the others disconnected), collected for the ticks to come; the preferences first,
    /// as every frame.
    fn set_input(&mut self, touch: &TouchState, held: PadHeld) {
        self.pad.set_input(touch, held);
        preferences(&mut self.sim);
        let pad = &self.pad;
        self.sim.input.collect(std::array::from_fn(|slot| if slot == 0 { pad.packet() } else { Err(DeviceError::Disconnected) }));
    }

    /// Off the board the stick is relative to what the player sees (the headset doesn't draw the game's camera): the
    /// native presentation basis, right being -X and at -Z of the view.
    fn set_view(&mut self, eyes: Option<[Vec3; 3]>) {
        self.sim.set_view(eyes.map(|[right, up, forward]| [(-right).to_array(), up.to_array(), forward.to_array()]));
    }

    /// Not mid-grind (the grind refers to the rails it has).
    fn set_stage(&mut self, stage: &Stage) -> Applied {
        match self.stage.set_stage(&mut self.sim.physics, &mut self.sim.skater, stage) {
            Ok(applied) => applied,
            Err(e) => {
                eprintln!("Skate 3: the stage: {e}");
                Applied::Deferred
            }
        }
    }

    fn place_player(&mut self, place: PlacePlayer) {
        if let Err(e) = stage::place_player(&mut self.sim.skater, place.point, place.forward) {
            eprintln!("Skate 3: placing the skater: {e}");
        }
    }

    fn step(&mut self) -> Result<(), String> {
        let observation = self.sim.tick(true)?;
        if let Some(frame) = self.sim.camera_frame() {
            model::pose(&self.sim.skater, frame, &mut self.pose);
            self.pose.model = SKATER;
            self.posed = true;
        }
        let scoring = &self.sim.skater.scoring;
        if let Some(hud) = &mut self.hud
            && let Err(e) = hud.advance(scoring.hud_input(), scoring.new_trick, scoring.modified_trick, scoring.close_tricks)
        {
            eprintln!("Skate 3: its scoring HUD stopped: {e}");
        }
        // (retail plays the multiplier's sounds from its HUD module, display or not: with the tick, so none is missed)
        let multiplier = sound::multiplier_sound(&self.sim.skater);
        match &mut self.sound {
            Sound::Own(feed, _) => {
                if let Some(id) = multiplier {
                    feed.front_end(id);
                }
                if let Some(o) = &observation {
                    feed.observe(o);
                }
            }
            Sound::Carried(heard) => {
                if let Some(o) = &observation {
                    heard.observe(o);
                }
                if multiplier.is_some() {
                    heard.front_end();
                }
            }
        }
        Ok(())
    }

    fn focus(&self) -> Option<Focus> {
        stage::link(&self.sim.physics, &self.sim.skater).focus()
    }

    fn models(&mut self) -> Vec<Model> {
        self.model.take().into_iter().collect()
    }

    fn poses(&mut self, out: &mut Vec<Pose>) {
        if std::mem::take(&mut self.posed) {
            out.push(self.pose.clone());
        }
    }

    fn stream(&mut self) -> Option<StreamFactory> {
        match &mut self.sound {
            Sound::Own(_, stream) => stream.take(),
            Sound::Carried(_) => None,
        }
    }

    /// Its trick display's frame when it changed.
    fn hud(&mut self, out: &mut HudFrame) -> bool {
        let Some(hud) = &mut self.hud else { return false };
        match hud.frame(out) {
            Ok(changed) => changed,
            Err(e) => {
                // (it stopped: its part shown empty from now on)
                eprintln!("Skate 3: its HUD's geometry: {e}");
                self.hud = None;
                true
            }
        }
    }

    fn hud_texture(&mut self, id: TextureId) -> Option<Rgba8> {
        self.hud.as_mut()?.texture(id)
    }

    fn set_setting(&mut self, key: &str, value: f32) {
        if key == settings::CIRCLE_ALWAYS
            && let Some(hud) = &mut self.hud
        {
            hud.set_circle_always(value > 0.5);
        }
    }

    fn skate(&mut self) -> Option<&mut dyn SkateMode> {
        Some(self)
    }
}

impl SkateMode for Skate3 {
    fn set_park(&mut self, park: &SkatePark) -> Applied {
        self.stage.set_park(park)
    }

    fn link(&self) -> Option<SkaterLink> {
        Some(stage::link(&self.sim.physics, &self.sim.skater))
    }

    fn trick(&self) -> Option<TrickText> {
        Some(trick::text(&self.sim.skater))
    }

    fn sounds(&mut self) -> Option<SkateSounds> {
        match &mut self.sound {
            Sound::Carried(heard) => Some(heard.take()),
            Sound::Own(..) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real game (`SKATE3_ASSET_ROOT`; skipped without it) skated through a line - pushed, an ollie, rolled on
    /// until the line ends - its trick display drawn as the engine has it (the circle always shown, by default) and,
    /// fed the same scoring, as retail has it: retail's circle comes and goes, the engine's is drawn throughout.
    #[test]
    fn the_circle_stays_shown_through_a_line() {
        let Some(assets) = std::env::var_os("SKATE3_ASSET_ROOT").map(PathBuf::from) else {
            eprintln!("SKATE3_ASSET_ROOT not set: skipped");
            return;
        };
        let mut engine = load(assets.clone(), Difficulty::Easy, false).expect("Skate 3");
        let mut retail = hud::Hud::load(&hud::root(&assets), engine.sim.skater.scoring.hud_input()).expect("its HUD");
        let (mut ours, mut theirs) = (Vec::new(), Vec::new());
        for t in 0..900u32 {
            let touch = TouchState {
                x: matches!(t, 20..=24 | 40..=44 | 60..=64),
                right_stick: match t {
                    120..=125 => [0.0, -1.0],
                    126..=128 => [0.0, 1.0],
                    _ => [0.0, 0.0],
                },
                ..Default::default()
            };
            engine.set_input(&touch, PadHeld::default());
            engine.step().expect("a tick");
            let scoring = &engine.sim.skater.scoring;
            retail.advance(scoring.hud_input(), scoring.new_trick, scoring.modified_trick, scoring.close_tricks).expect("retail's");
            ours.push(engine.hud.as_mut().expect("its HUD").circle_alpha());
            theirs.push(retail.circle_alpha());
        }
        let shown = |a: &[f32]| a.iter().filter(|&&x| x > 0.9).count();
        let hidden = |a: &[f32]| a.iter().filter(|&&x| x < 0.05).count();
        eprintln!(
            "retail's circle: shown {} hidden {} of {}; ours: shown {} hidden {}",
            shown(&theirs),
            hidden(&theirs),
            theirs.len(),
            shown(&ours),
            hidden(&ours)
        );
        eprintln!("retail's alpha every 30 ticks: {:?}", theirs.iter().step_by(30).collect::<Vec<_>>());
        assert!(shown(&theirs) > 0 && hidden(&theirs) > 0, "retail's comes and goes");
        assert_eq!(hidden(&ours), 0, "ours is always drawn: {:?}", ours.iter().step_by(30).collect::<Vec<_>>());
    }

    #[test]
    fn it_moves_to_the_runtimes_worker() {
        fn send<T: Send>() {}
        send::<super::Skate3>();
    }
}
