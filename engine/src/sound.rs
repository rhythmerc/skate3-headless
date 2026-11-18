//! Skate 3's sound, two ways (the app plays one):
//!
//! - **Its own** ([`stream`]): the sim's recovered rider and board sound graph (`skate3_sim::skate_audio::player`, on
//!   the player's decoded sound banks) as an [`engine_core::audio::Stream`]. Each tick's observation goes to it through
//!   a [`StreamFeed`]; the graph renders on the app's audio thread.
//! - **The skating sounds the app carries** ([`Heard`]): each tick's observation said as the skate mode's
//!   [`SkateSounds`] (Skate 3's state numbers and trick names are only known here).
//!
//! Retail's score-multiplier sounds are front-end sounds ([`multiplier_sound`]): through the stream, or a chime.
use engine_core::audio::{Stream, StreamFactory};
use engine_core::glam::Vec3;
use engine_core::skate::sound::{Cue, Rolling, SkateSounds};
use skate_core::physics::phase::PhysicsEvent;
use skate3_sim::physics::SkaterRuntime;
use skate3_sim::skate_audio::PlayerAudioObservation;
use skate3_sim::skate_audio::player::{FRAME_SECONDS, OBSERVATION_QUEUE, OUTPUT_CHANNELS, PlayerAudio, SAMPLE_RATE};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::time::Duration;

/// The physical states (`PhysicalStateId`) the sounds tell apart.
fn is_ground(state: u32) -> bool {
    (100..=105).contains(&state)
}

fn is_air(state: u32) -> bool {
    (200..=202).contains(&state)
}

fn is_grind(state: u32) -> bool {
    (400..=405).contains(&state)
}

fn is_walking(state: u32) -> bool {
    (500..=503).contains(&state)
}

/// Whether a trick (its scoring identifier) flips or spins the board: the whoosh and catch.
pub fn is_flip(trick: &str) -> bool {
    let t = trick.to_ascii_lowercase();
    ["flip", "heel", "shuv", "varial", "tre", "360", "impossible", "casper", "dolphin"].iter().any(|k| t.contains(k))
}

/// A push this fast (m/s) is heard.
const PUSH_SPEED: f32 = 0.35;
/// A footstep this strong is heard.
const FOOTSTEP_STRENGTH: f32 = 0.1;

/// What the last tick said, for edges.
#[derive(Clone, Default)]
struct Edges {
    state: u32,
    wiping_out: bool,
    pushing: bool,
    stepping: bool,
    trick: Option<String>,
}

/// Skate 3's ticks as [`SkateSounds`]: the cues since the app last took them, and how the skater is after the last.
#[derive(Default)]
pub struct Heard {
    previous: Option<Edges>,
    sounds: SkateSounds,
}

impl Heard {
    /// A completed tick.
    pub fn observe(&mut self, o: &PlayerAudioObservation) {
        let prev = self.previous.take().unwrap_or(Edges { state: o.state, ..Default::default() });
        let cues = &mut self.sounds.cues;
        let wiped = o.wiping_out || o.events.iter().any(|e| matches!(e, PhysicsEvent::Wipeout));
        // Pop: leaving the ground or a grind for the air, by the skater's own doing.
        if is_air(o.state) && (is_ground(prev.state) || is_grind(prev.state)) && !wiped {
            cues.push(Cue::Pop);
        }
        if o.landed || o.events.iter().any(|e| matches!(e, PhysicsEvent::Landing)) {
            cues.push(Cue::Land { impact: o.landing_impact_speed });
        }
        if o.trick_identifier != prev.trick && is_air(o.state) && o.trick_identifier.as_deref().is_some_and(is_flip) {
            cues.push(Cue::Flip);
        }
        let pushing = o.foot_push_speed.abs() > PUSH_SPEED;
        if pushing && !prev.pushing {
            cues.push(Cue::Push);
        }
        let stepping = o.footstep_strength > FOOTSTEP_STRENGTH && is_walking(o.state);
        if stepping && !prev.stepping {
            cues.push(Cue::Footstep { strength: o.footstep_strength });
        }
        if wiped && !prev.wiping_out {
            cues.push(Cue::Bail);
        }
        self.previous = Some(Edges { state: o.state, wiping_out: wiped, pushing, stepping, trick: o.trick_identifier.clone() });
        let s = &mut self.sounds;
        s.at = crate::stage::from_sim(Vec3::from_array(o.retail.position));
        s.speed = o.board_speed;
        s.rolling = if o.grinding || is_grind(o.state) {
            Rolling::Grind
        } else if o.grounded && is_ground(o.state) && !o.wiping_out {
            Rolling::Ground
        } else {
            Rolling::None
        };
        s.powersliding = o.powersliding;
        s.airborne = is_air(o.state);
    }

    /// A front-end sound (retail's `GlobalFEPlaySound`: the multiplier's), heard as one chime however many come
    /// together.
    pub fn front_end(&mut self) {
        if !self.sounds.cues.contains(&Cue::Chime) {
            self.sounds.cues.push(Cue::Chime);
        }
    }

    /// The sounds since the last take: the cues go, the skater's state stays.
    pub fn take(&mut self) -> SkateSounds {
        let cues = std::mem::take(&mut self.sounds.cues);
        SkateSounds { cues, ..self.sounds.clone() }
    }
}

/// Retail's sound for the score multiplier that changed this tick (`sub_82666BC0`: the scoring keeps its cached copy
/// and reports the change; the choice is by exact value), if it has one. Run it every tick, so no change is missed.
pub fn multiplier_sound(skater: &SkaterRuntime) -> Option<u64> {
    skater.scoring.multiplier_changed.and_then(skate3_sim::skate_audio::player::multiplier_sound_id)
}

/// The decoded sound banks are there to play retail's own sound from.
pub fn banks_decoded() -> bool {
    skate3_sim::skate_audio::player::banks_decoded()
}

/// The sim's own sound graph, from the prepared data's `assets`: the feed the ticks go into, and the factory that
/// makes the stream on the app's audio thread (decoding the banks there).
pub fn stream(assets: std::path::PathBuf) -> (StreamFeed, StreamFactory) {
    let (input, observations) = mpsc::sync_channel(OBSERVATION_QUEUE);
    let factory: StreamFactory = Box::new(move || {
        let audio = PlayerAudio::prepare(&assets)?;
        eprintln!("SKATE_PLAYER_AUDIO ready samples={} cache_hits={}", audio.samples, audio.cache_hits);
        Ok(Box::new(Retail { audio, ticks: Feed { observations, last: None, carry: 0.0 } }) as Box<dyn Stream>)
    });
    (StreamFeed { input, at: None, front_end: Vec::new(), dropped: 0 }, factory)
}

/// Where Skate 3's ticks go to its own sound.
pub struct StreamFeed {
    input: SyncSender<PlayerAudioObservation>,
    at: Option<Vec3>,
    front_end: Vec<u64>,
    dropped: u64,
}

impl StreamFeed {
    /// A completed tick, with the front-end sounds since the last. The game never waits for the sound: with the
    /// stream behind, the tick is dropped (counted: [`StreamFeed::dropped`]).
    pub fn observe(&mut self, observation: &PlayerAudioObservation) {
        self.at = Some(crate::stage::from_sim(Vec3::from_array(observation.retail.position)));
        let mut observation = observation.clone();
        observation.frontend_sounds.append(&mut self.front_end);
        match self.input.try_send(observation) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(lost)) => {
                self.dropped += 1;
                eprintln!(
                    "SKATE_PLAYER_AUDIO observation_overflow tick={} footstep={} push={} landed={} events={:?}",
                    lost.tick, lost.footstep_strength, lost.foot_push_speed, lost.landed, lost.events
                );
            }
        }
    }

    /// A front-end sound (by its AttribSys id), played with the next tick: retail calls `GlobalFEPlaySound` from its
    /// UI, off the simulation's schedule.
    pub fn front_end(&mut self, id: u64) {
        self.front_end.push(id);
    }

    /// Where the sound is heard from: the skater, as the last tick had it.
    pub fn at(&self) -> Option<Vec3> {
        self.at
    }

    /// The ticks dropped since the last ask.
    pub fn dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }
}

/// The ticks as the graph takes them: one a block at most, in order (a tick may hold a one-tick contact or landing:
/// none is skipped; at about 188 blocks a second the graph catches up with 60 ticks). While the game is held, retail
/// goes on publishing its frozen state each frame with `paused` set (its pause silences the player's sounds through
/// that input): the last tick again at the tick rate, so marked.
struct Feed {
    observations: Receiver<PlayerAudioObservation>,
    last: Option<PlayerAudioObservation>,
    carry: f32,
}

impl Feed {
    fn next(&mut self, paused: bool, block: f32) -> Result<Option<PlayerAudioObservation>, String> {
        match self.observations.try_recv() {
            Ok(observation) => {
                // (what's repeated is the tick's state: its front-end sounds were played once)
                self.last = Some(PlayerAudioObservation { frontend_sounds: Vec::new(), ..observation.clone() });
                return Ok(Some(observation));
            }
            Err(TryRecvError::Disconnected) => return Err("player audio observation source disconnected".into()),
            Err(TryRecvError::Empty) => {}
        }
        if !paused {
            self.carry = 0.0;
            return Ok(None);
        }
        self.carry += block;
        if self.carry < FRAME_SECONDS {
            return Ok(None);
        }
        self.carry -= FRAME_SECONDS;
        Ok(self.last.clone().map(|mut frozen| {
            frozen.retail.paused = true;
            frozen
        }))
    }
}

struct Retail {
    audio: PlayerAudio,
    ticks: Feed,
}

impl Stream for Retail {
    fn channels(&self) -> u16 {
        OUTPUT_CHANNELS
    }

    fn rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn block(&self) -> Duration {
        PlayerAudio::block_duration()
    }

    fn render(&mut self, paused: bool) -> Result<Vec<f32>, String> {
        let next = self.ticks.next(paused, PlayerAudio::block_duration().as_secs_f32())?;
        self.audio.render_block(next.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(n: u64) -> PlayerAudioObservation {
        let mut o = crate::sound::tests_support::observation();
        o.tick = n;
        o
    }

    #[test]
    fn ticks_go_in_order_and_a_held_game_repeats_the_last_at_the_tick_rate() {
        let (input, observations) = mpsc::sync_channel(OBSERVATION_QUEUE);
        let mut feed = Feed { observations, last: None, carry: 0.0 };
        let block = PlayerAudio::block_duration().as_secs_f32();
        let mut first = tick(10);
        first.frontend_sounds.push(7);
        for o in [first, tick(11), tick(12)] {
            input.send(o).unwrap();
        }
        // (several ticks in one frame: each its own block, none dropped)
        let got: Vec<_> = (0..4).map(|_| feed.next(false, block).unwrap().map(|o| (o.tick, o.frontend_sounds))).collect();
        assert_eq!(got, [Some((10, vec![7])), Some((11, vec![])), Some((12, vec![])), None]);
        // Held: the last tick again, paused, once a tick's time (60 Hz from ~188 blocks a second).
        let held: Vec<_> = (0..188).filter_map(|_| feed.next(true, block).unwrap()).collect();
        assert!((59..=61).contains(&held.len()), "{}", held.len());
        assert!(held.iter().all(|o| o.tick == 12 && o.retail.paused && o.frontend_sounds.is_empty()));
        // Going on: no more repeats, and a new tick first.
        input.send(tick(13)).unwrap();
        assert_eq!(feed.next(false, block).unwrap().map(|o| o.tick), Some(13));
        assert_eq!(feed.next(false, block).unwrap(), None);
        drop(input);
        assert!(feed.next(false, block).is_err());
    }

    /// The real graph (`SKATE3_ASSET_ROOT`: the prepared data's `assets`, and `LOCALAPPDATA` holding its decoded banks,
    /// `Skate3RustEngine/audio-pcm-cache`; skipped without them): made by the
    /// factory, fed ticks through the feed (a landing among them), held, and rendered a second's blocks, each a full
    /// block of finite stereo. (Silent: these ticks lack the sim's physical state retail's graph sounds from.)
    #[test]
    fn the_real_graph_renders_from_its_feed() {
        let Some(assets) = std::env::var_os("SKATE3_ASSET_ROOT").filter(|_| banks_decoded()) else {
            eprintln!("SKATE3_ASSET_ROOT not set, or no decoded banks under LOCALAPPDATA: skipped");
            return;
        };
        let (mut feed, factory) = stream(assets.into());
        let mut stream = factory().expect("the graph");
        assert_eq!((stream.channels(), stream.rate()), (OUTPUT_CHANNELS, SAMPLE_RATE));
        let frames = (stream.block().as_secs_f64() * f64::from(SAMPLE_RATE)).round() as usize;
        let (mut blocks, mut loud) = (0, 0.0f32);
        for n in 0..300u64 {
            if n % 3 == 0 && n < 240 {
                let mut o = tick(n / 3);
                if n == 90 {
                    o.landed = true;
                    o.landing_impact_speed = -3.0;
                    o.events.push(PhysicsEvent::Landing);
                }
                feed.observe(&o);
            }
            let block = stream.render(n >= 240).expect("a block");
            assert_eq!(block.len(), frames * usize::from(OUTPUT_CHANNELS));
            assert!(block.iter().all(|v| v.is_finite()));
            loud = block.iter().fold(loud, |m, v| m.max(v.abs()));
            blocks += 1;
        }
        assert_eq!(feed.dropped(), 0);
        eprintln!("skate3 graph: {blocks} blocks of {frames} frames, peak {loud}");
    }

    #[test]
    fn flips_are_recognised() {
        assert!(is_flip("Kickflip"));
        assert!(is_flip("varial_heelflip"));
        assert!(!is_flip("Ollie"));
    }
}

/// An observation with nothing in it, for tests here and in the app.
#[doc(hidden)]
pub mod tests_support {
    use skate3_sim::skate_audio::{PlayerAudioObservation, RetailAudioInputs};

    pub fn observation() -> PlayerAudioObservation {
        PlayerAudioObservation {
            tick: 0,
            state: 100,
            board_speed: 0.0,
            rider_speed: 0.0,
            grounded: true,
            grinding: false,
            grind_family: 0,
            grind_substate: 0,
            grind_audio_surface: 0,
            grind_impact_speed: 0.0,
            wiping_out: false,
            landed: false,
            landing_impact_speed: 0.0,
            landing_clean: false,
            landing_sketchy: false,
            landing_type: 0,
            landing_spin: 0.0,
            landing_sideways_speed: 0.0,
            footstep_strength: 0.0,
            footstep_bone: -1,
            foot_push_speed: 0.0,
            feet_supported: [false; 2],
            foot_surface: 0,
            contact_count: 0,
            powersliding: false,
            trick_id: None,
            trick_identifier: None,
            animation_name: None,
            riding_switch: false,
            riding_fakie: false,
            nollie: false,
            wheel_surface: 0,
            events: Vec::new(),
            frontend_sounds: Vec::new(),
            retail: RetailAudioInputs::default(),
        }
    }
}
