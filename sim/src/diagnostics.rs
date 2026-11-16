//! A ring of the latest physical-state transitions, for a host's crash report (skate-game's
//! `crash_context` formats it once a second). Recording copies three scalars and never blocks.
use skate_core::player::state::PhysicalStateId;

pub type Transition = (u64, PhysicalStateId, PhysicalStateId);

/// (ring, next slot, total recorded).
pub static EVENTS: std::sync::Mutex<([Option<Transition>; 64], usize, u64)> =
    std::sync::Mutex::new(([None; 64], 0, 0));

pub fn requested(tick: u64, from: PhysicalStateId, to: PhysicalStateId) {
    if let Ok(mut events) = EVENTS.try_lock() {
        let index = events.1;
        events.0[index] = Some((tick, from, to));
        events.1 = (index + 1) % 64;
        events.2 += 1;
    }
}
