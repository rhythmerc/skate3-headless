//! Off-board game adapters over the shared physical and scene owners.
//!
//! These modules are the source-backed implementation units for the off-board
//! owner. The coordinator calls them through the selected physical state.
#![allow(dead_code)]
pub mod air_feet;
pub mod air_selector;
pub mod board_manager;
pub mod board_possession;
pub mod contact_toolkit;
pub mod grab_scene;
pub mod ground_geometry;
pub mod ground_query;
pub mod ground_sync;
pub mod landing_deck;
pub mod pose_adjust;
pub mod post_physics;
pub mod settings;
pub mod skeleton_air;
pub mod skeleton_ground;

pub mod contact_queries;
