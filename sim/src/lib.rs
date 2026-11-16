//! The Skate 3 simulation with no Bevy: the faithful port's physics, animation graphs, scoring,
//! APT HUD runtime and player-audio graph, as plain structs. The app (`skate-game`) owns the
//! schedule, rendering and audio output; see `docs/skate3-sim-split.md`.
//!
//! Module paths mirror where the code lived in `skate-game/src`, so `crate::physics::...` inside
//! the sim and the app's re-exports at the old paths both keep resolving.
//!
//! The stable surface is the façade: [`Bundle`], [`Sim`] (load, staged step, outputs) and
//! [`PlayerAudio`] (observations in, mixed PCM out). The modules underneath are public too while
//! the app still reads sim internals directly (docs/skate3-sim-split.md S4/S7).

pub use sim::{Bundle, Link, Parts, Sim, SimConfig, SkaterPose};
pub use skate_audio::{PlayerAudioObservation, player::PlayerAudio};

pub mod animation;
pub mod animation_pose;
pub mod apt_display;
pub mod apt_movie;
pub mod apt_scene;
pub mod apt_text;
pub mod apt_vm;
pub mod camera;
pub mod diagnostics;
pub mod difficulty;
pub mod graph_host;
pub mod graph_runtime;
pub mod grind_world;
pub mod hud_runtime;
pub mod input;
pub mod map_library;
pub mod physics;
pub mod prelude;
pub mod scoring_runtime;
pub mod sim;
pub mod skate_audio;
pub mod skate_world;
pub mod skater_animation;

#[cfg(test)]
#[path = "tests/action_host.rs"]
mod action_host_tests;
