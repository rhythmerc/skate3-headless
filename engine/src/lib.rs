//! **Skate 3 only.** Skate 3's side of the engine contract ([`engine_core`]): what the app needs to know about Skate 3
//! to host it, said as the contract's plain data, over its simulation (`skate3-sim`). No Bevy.
//!
//! It lives in skate3-headless beside `skate3-sim`; a host (skate3-quest-ar) depends on it by revision. Nothing here
//! depends on a host.
//!
//! - [`engine`]: Skate 3 as the runtime drives it ([`engine_core::EngineCore`]), made on a worker by its factory.
//! - [`hud`]: the retail trick display (its movie, read from the player's prepared HUD files) as a
//!   [`engine_core::hud::HudFrame`].
//! - [`input`]: the Touch controllers as the Xbox pad the sim reads, and its controls sheet.
//! - [`model`]: the skater's model (its GLB bound to the sim's rig) and its pose after each tick, cuts included.
//! - [`stage`]: the stage and park as the sim's collision and rails (not mid-grind), putting the skater down, and
//!   where the rider and the board are.
//! - [`sound`]: its own sound graph as a stream, or its ticks as the skating sounds the app carries, and retail's
//!   multiplier sounds.
//! - [`trick`]: the sim's scoring as the plain trick readout.
//! - [`DATA`]: the player's Skate 3 data, as the app's installer finds and keeps it.
pub mod engine;
pub mod hud;
pub mod input;
pub mod model;
pub mod sound;
pub mod stage;
pub mod trick;

/// The player's Skate 3 data: what the desktop setup makes from their own disc (skate3-quest-ar's `docs/IMPORT-SKATE3.md`), an `assets`
/// folder with `private/game.json` and, beside it, the decoded sound banks (`audio-pcm-cache`: the headset can't
/// decode them itself).
pub const DATA: engine_core::GameFiles = engine_core::GameFiles {
    drop: "Skate3",
    folder: "assets",
    marker: "private/game.json",
    beside: &[("audio-pcm-cache", "Skate3RustEngine/audio-pcm-cache")],
    how: &[
        "On a Windows PC, once: run skate3rust.exe on your own Skate 3",
        "ISO, skate a minute, then double-click MAKE-QUEST-ZIP.bat.",
        "Copy Skate3-Quest.zip to Download/Skate3 on this headset,",
        "then import it here. Guide: docs/IMPORT-SKATE3.md on GitHub.",
    ],
};
