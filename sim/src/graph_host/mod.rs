//! Production graph host building blocks. Root owns schedule and registration.
// Offboard graph leaves remain parsed/source-backed but are not registered as
// live state owners until the corresponding Biped services are integrated.
#![allow(dead_code)]
pub mod action;
mod action_conditions;
pub mod action_nodes;
mod condition_nodes;
pub mod motion_manual;
pub mod outputs;
pub use condition_nodes::numeric as parse_numeric_condition;
mod crouching_settings;
/// Deliberate non-retail extension; inert unless a mod enables it.
pub mod endless_flip;
pub mod motion;
mod motion_air_leg;
mod motion_animation;
mod motion_bump;
mod motion_channels;
pub mod motion_character_gesture;
mod motion_conditions;
#[path = "motion_offboard/dismount.rs"]
mod motion_dismount;
mod motion_finger_flip;
pub mod motion_gameplay_conditions;
pub mod motion_grind;
pub mod motion_ground_slope;
pub mod motion_hand_services;
mod motion_hippy_jump;
mod motion_hooks;
pub mod motion_intent_filter;
mod motion_kickturn;
pub mod motion_landing;
mod motion_landing_execute;
pub mod motion_native;
mod motion_nodes;
#[path = "motion_offboard/cadence.rs"]
pub mod motion_offboard_cadence;
#[path = "motion_offboard/push_off.rs"]
pub mod motion_push_off;
#[path = "motion_offboard/reset.rs"]
pub mod motion_reset;
mod motion_riding;
pub mod motion_riding_conditions;
#[path = "motion_offboard/runout.rs"]
pub mod motion_runout;
mod motion_scoring_trick;
pub mod motion_shove;
mod motion_sliding;
pub mod motion_spin;
pub mod motion_stock_conditions;
pub mod motion_stock_gameplay;
#[path = "motion_offboard/toggle_board.rs"]
pub mod motion_toggle_board;
mod motion_tricks;
#[path = "motion_offboard/twist_lean.rs"]
pub mod motion_twist_lean;
pub mod motion_wipeout;
mod pumping_settings;
pub mod pushing;
pub mod pushing_settings;
mod turning_settings;
