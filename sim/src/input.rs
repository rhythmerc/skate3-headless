//! Pad history, gesture recognition and the gameplay action map. Device polling stays with the
//! host, which hands each slot's [`device::DevicePacket`] to [`ControllerInput::collect`].
mod controllers;
pub mod device;
pub mod gesture_catalog;
pub mod gesture_input;
pub mod gesture_mapping;
mod gesture_mapping_data;
pub use controllers::{ControllerInput, ControllerStatus};

#[cfg(test)]
pub mod manual_replay;
