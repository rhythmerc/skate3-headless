//! ZIP grind runtime split into source-closed components against CURRENT APIs.
//! Physical acquisition remains with the input manager. The retained runtime
//! owns family state, body operations and publication, never candidate selection.
pub mod acquisition;
mod advance;
mod arithmetic;
pub mod board;
mod condition;
mod contact;
pub mod forces;
mod involuntary;
pub mod launch;
pub mod lifecycle;
pub mod observation;
pub mod output;
mod phase;
pub mod post;
mod rng;
mod runtime;
mod settings;
mod state;
mod substate;
mod tip_nudge;
mod wipeout_output;
pub use advance::advance;
pub use condition::condition;
pub use phase::{enter, exit, fill, post};
pub use runtime::Runtime;

pub use observation::ManagerObservation;

/// Physical/output family, NOT the seven-valued admission kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Family {
    FiftyFifty = 0,
    Boardslide = 1,
    Tipslide = 2,
    FiveO = 3,
    Backslash = 4,
    Darkslide = 5,
}

impl Family {
    pub fn from_physical_state(state: u32) -> Option<Self> {
        Some(match state {
            400 => Self::Boardslide,
            401 => Self::FiftyFifty,
            402 => Self::Tipslide,
            403 => Self::FiveO,
            404 => Self::Backslash,
            405 => Self::Darkslide,
            _ => return None,
        })
    }
    pub fn physical_state(self) -> u32 {
        match self {
            Self::Boardslide => 400,
            Self::FiftyFifty => 401,
            Self::Tipslide => 402,
            Self::FiveO => 403,
            Self::Backslash => 404,
            Self::Darkslide => 405,
        }
    }
}
