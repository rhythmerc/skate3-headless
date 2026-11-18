//! Quest Touch controllers presented as the Xbox pad the sim's input pipeline was built for. Everything downstream
//! (the TU3 conversion in `skate_core::input::xbox`, gesture mapping, flick-it) sees an ordinary `XboxState`, so
//! skating on Touch uses the exact same code as on a pad.
//!
//! Face buttons, sticks, triggers and stick clicks map one-to-one (the Touch labels A/B/X/Y match the Xbox ones); the
//! grips are the bumpers. The left menu button is the runtime's (a tap is its menu, a hold build mode), so while it
//! is held the left stick and X do not reach the game. Touch has no D-pad, Back or Start, and nothing in the game
//! needs them on the headset.
use engine_core::{Control, PadHeld, TouchState};
use skate_core::input::xbox::XboxState;
use skate3_sim::input::device::{DeviceError, DevicePacket};

const LEFT_THUMB: u16 = 0x0040;
const RIGHT_THUMB: u16 = 0x0080;
const LEFT_SHOULDER: u16 = 0x0100;
const RIGHT_SHOULDER: u16 = 0x0200;
const A: u16 = 0x1000;
const B: u16 = 0x2000;
const X: u16 = 0x4000;
const Y: u16 = 0x8000;

/// A grip counts as a held bumper past this pull.
const GRIP_PRESSED: f32 = 0.5;

/// The game's controls on Touch (the runtime lists its own - its menu, placing the skater - after these).
pub const CONTROLS: &[Control] = &[
    Control::new("Right stick", "flick it: ollies and flip tricks"),
    Control::new("Left stick", "steer; in the air, spin"),
    Control::new("A / X", "push / mongo push"),
    Control::new("B", "brake"),
    Control::new("Y", "off / on the board"),
    Control::new("Triggers", "grabs (LT / RT)"),
    Control::new("Grips", "LB / RB"),
    Control::new("Stick clicks", "LS / RS"),
];

/// The pad at rest.
pub const NEUTRAL: XboxState = XboxState { buttons: 0, triggers: [0, 0], left: [0, 0], right: [0, 0] };

/// The Xbox pad state for the Touch controllers' state.
pub fn touch_to_pad(touch: &TouchState) -> XboxState {
    let mut buttons = 0u16;
    // The menu button's chords are the runtime's: neither the stick nor X leaks into the game.
    let (left, x) = if touch.menu { ([0.0, 0.0], false) } else { (touch.left_stick, touch.x) };
    for (pressed, bit) in [
        (touch.a, A),
        (touch.b, B),
        (x, X),
        (touch.y, Y),
        (touch.left_stick_click, LEFT_THUMB),
        (touch.right_stick_click, RIGHT_THUMB),
        (touch.left_grip >= GRIP_PRESSED, LEFT_SHOULDER),
        (touch.right_grip >= GRIP_PRESSED, RIGHT_SHOULDER),
    ] {
        if pressed {
            buttons |= bit;
        }
    }
    XboxState {
        buttons,
        triggers: [trigger(touch.left_trigger), trigger(touch.right_trigger)],
        left: stick(left),
        right: stick(touch.right_stick),
    }
}

fn trigger(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn stick([x, y]: [f32; 2]) -> [i16; 2] {
    // XInput's range is asymmetric (-32768..=32767); full deflection either way lands on the ends the TU3 conversion
    // normalises by 1/32768.
    let axis = |v: f32| {
        let v = v.clamp(-1.0, 1.0);
        if v < 0.0 { (v * 32768.0).round() as i16 } else { (v * 32767.0).round() as i16 }
    };
    [axis(x), axis(y)]
}

/// The pad in controller slot 0, as the sim's input reads it: the Touch controllers. Disconnected until it is first
/// given a state; each change is a new packet.
#[derive(Default, Debug)]
pub struct ExternalPad {
    state: Option<(u16, [u8; 2], [i16; 2], [i16; 2])>,
    packet: u32,
}

impl ExternalPad {
    /// This frame's controllers: [`touch_to_pad`], neutral while the runtime has them. (While it only pauses, the
    /// runtime holds the sim's tick itself.)
    pub fn set_input(&mut self, touch: &TouchState, held: PadHeld) {
        self.set(if held.held { NEUTRAL } else { touch_to_pad(touch) });
    }

    pub fn set(&mut self, state: XboxState) {
        let next = Some((state.buttons, state.triggers, state.left, state.right));
        if next != self.state {
            self.packet = self.packet.wrapping_add(1);
            self.state = next;
        }
    }

    /// The packet slot 0 reads now.
    pub fn packet(&self) -> Result<DevicePacket, DeviceError> {
        let (buttons, triggers, left, right) = self.state.ok_or(DeviceError::Disconnected)?;
        Ok(DevicePacket {
            number: self.packet,
            state: XboxState { buttons, triggers, left, right },
            // XINPUT_DEVSUBTYPE_GAMEPAD.
            subtype: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default() -> TouchState {
        TouchState::default()
    }

    fn fields(s: XboxState) -> (u16, [u8; 2], [i16; 2], [i16; 2]) {
        (s.buttons, s.triggers, s.left, s.right)
    }

    #[test]
    fn buttons_sticks_and_triggers_map_one_to_one() {
        let state = touch_to_pad(&TouchState {
            left_stick: [-1.0, 1.0],
            right_stick: [0.5, -0.25],
            left_trigger: 1.0,
            right_trigger: 0.5,
            left_grip: 0.9,
            right_grip: 0.2,
            a: true,
            y: true,
            right_stick_click: true,
            ..default()
        });
        assert_eq!(state.buttons, A | Y | RIGHT_THUMB | LEFT_SHOULDER);
        assert_eq!(state.left, [-32768, 32767]);
        assert_eq!(state.right, [16384, -8192]);
        assert_eq!(state.triggers, [255, 128]);
    }

    #[test]
    fn out_of_range_runtime_values_are_clamped() {
        let state = touch_to_pad(&TouchState { left_stick: [1.2, -3.0], left_trigger: -0.1, right_trigger: 1.5, ..default() });
        assert_eq!(state.left, [32767, -32768]);
        assert_eq!(state.triggers, [0, 255]);
    }

    #[test]
    fn menu_held_keeps_the_left_stick_and_x_from_the_game() {
        let held = touch_to_pad(&TouchState { menu: true, left_stick: [0.9, 0.0], x: true, a: true, ..default() });
        assert_eq!(held.buttons, A, "only the menu's chord inputs are held back");
        assert_eq!(held.left, [0, 0], "stick does not also steer");
        let released = touch_to_pad(&TouchState { left_stick: [0.9, 0.0], x: true, ..default() });
        assert_eq!(released.buttons, X);
        assert_ne!(released.left, [0, 0]);
    }

    /// Touch to slot 0's packet, as `set_input` gives it: neutral (but connected) while the runtime has the controllers,
    /// a new packet only when the state changes.
    #[test]
    fn touch_to_pad_and_neutral_while_held() {
        let mut pad = ExternalPad::default();
        assert!(matches!(pad.packet(), Err(DeviceError::Disconnected)), "nothing given yet");
        let pushed = TouchState { left_stick: [0.0, 1.0], right_stick: [0.2, 0.0], a: true, right_trigger: 1.0, ..default() };
        let cases = [
            (pushed, PadHeld::default(), fields(touch_to_pad(&pushed))),
            (pushed, PadHeld { held: true, pause: false }, fields(NEUTRAL)),
            (pushed, PadHeld { held: true, pause: true }, fields(NEUTRAL)),
            (pushed, PadHeld { held: false, pause: true }, fields(touch_to_pad(&pushed))),
            (TouchState { menu: true, ..pushed }, PadHeld::default(), (A, [0, 255], [0, 0], [6553, 0])),
        ];
        let mut number = 0;
        for (i, (touch, held, want)) in cases.into_iter().enumerate() {
            pad.set_input(&touch, held);
            let packet = pad.packet().expect("connected");
            assert_eq!(fields(packet.state), want, "case {i}");
            assert_eq!(packet.subtype, 1);
            assert!(packet.number >= number, "case {i}");
            number = packet.number;
        }
        assert_eq!(fields(touch_to_pad(&pushed)), (A, [0, 255], [0, 32767], [6553, 0]));
        // (the same state again: the same packet)
        pad.set_input(&pushed, PadHeld::default());
        let n = pad.packet().unwrap().number;
        pad.set_input(&pushed, PadHeld::default());
        assert_eq!(pad.packet().unwrap().number, n);
    }
}
