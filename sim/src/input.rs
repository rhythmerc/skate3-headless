//! Pad history, gesture recognition and the gameplay action map. Device polling stays with the
//! host, which hands each slot's [`device::DevicePacket`] to [`ControllerInput::collect`].
mod controllers;
pub mod device;
pub mod gesture_catalog;
pub mod gesture_input;
pub mod gesture_mapping;
mod gesture_mapping_data;
pub use controllers::{ControllerInput, ControllerStatus};

/// The tick's first stage: drain the pads collected since the last tick into this tick's
/// gameplay actions. With `gameplay` false (a menu is open) gameplay presses are discarded first.
pub fn publish(input: &mut ControllerInput, gameplay: bool) -> skate_core::input::tick::TickInput {
    if !gameplay {
        input.discard_gameplay();
    }
    input.publish_actions();
    input.tick_input()
}

/// What a tick sees before any pad has been published.
pub fn idle_tick_input() -> skate_core::input::tick::TickInput {
    skate_core::input::tick::TickInput::new(
        0,
        skate_core::input::gameplay_map::GameplayActions::from_values([0.0; 18]),
        false,
    )
}

#[cfg(test)]
pub mod manual_replay;
