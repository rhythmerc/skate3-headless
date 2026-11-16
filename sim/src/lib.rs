//! The Skate 3 simulation with no Bevy: the faithful port's physics, animation graphs, scoring,
//! APT HUD runtime and player-audio graph, as plain structs. The app (`skate-game`) owns the
//! schedule, rendering and audio output; see `docs/skate3-sim-split.md`.
//!
//! Module paths mirror where the code lived in `skate-game/src`, so `crate::physics::...` inside
//! the sim and the app's re-exports at the old paths both keep resolving.

pub mod animation;
pub mod animation_pose;
pub mod apt_display;
pub mod apt_movie;
pub mod apt_scene;
pub mod apt_text;
pub mod apt_vm;
pub mod difficulty;
pub mod hud_runtime;
pub mod input;
pub mod map_library;
pub mod scoring_runtime;
pub mod skate_world;
