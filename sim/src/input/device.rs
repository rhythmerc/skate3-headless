//! What a pad poll returns, for [`super::ControllerInput::collect`]. The polling itself (XInput,
//! the headset's Touch controllers) is the host's.
use skate_core::input::xbox::XboxState;

pub struct DevicePacket {
    pub number: u32,
    pub state: XboxState,
    pub subtype: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceError {
    Disconnected,
    State(u32),
    Capabilities(u32),
    #[cfg(not(windows))]
    UnsupportedPlatform,
}
