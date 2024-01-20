use libc::timeval;
// use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use std::{
    mem::MaybeUninit,
    os::{fd::IntoRawFd, unix::fs::OpenOptionsExt},
};
use uinput_sys::{
    input_event, input_id, ui_dev_create, ui_set_absbit, ui_set_evbit, ui_set_keybit,
    uinput_user_dev, ABS_CNT, ABS_RX, ABS_RY, ABS_X, ABS_Y, BTN_A, BTN_B, BTN_X, BTN_Y, EV_ABS,
    EV_KEY, EV_SYN, UINPUT_MAX_NAME_SIZE,
};

use crate::util::write_type;

/// Represents a file descriptor associated with a `uinput` gamepad device.
pub struct GamepadFd {
    fd: i32,
    left_x: StickAxisState,
    left_y: StickAxisState,
}

#[derive(Debug)]
pub enum ButtonState {
    Release,
    Press,
    Repeat,
}

impl From<i32> for ButtonState {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Release,
            1 => Self::Press,
            2 => Self::Repeat,
            _ => unreachable!(),
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum GamepadButton {
    A = BTN_A,
    B = BTN_B,
    X = BTN_X,
    Y = BTN_Y,
}

pub fn map_button(key: i32) -> Option<GamepadButton> {
    use uinput_sys::*;
    Some(match key {
        KEY_ENTER | KEY_SPACE => GamepadButton::A,
        KEY_G => GamepadButton::Y,
        KEY_E => GamepadButton::X,
        KEY_C => GamepadButton::B,
        _ => return None,
    })
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(i16)]
pub enum StickAxisState {
    /// Bottom/Right
    High = i16::MAX,
    /// Centre
    #[default]
    Neutral = 0,
    /// Up/Left
    Low = i16::MIN,
}

#[derive(Debug, Clone, Copy)]
pub enum LeftStickAction {
    X(StickAxisState),
    Y(StickAxisState),
}

pub fn map_stick(key: i32, state: ButtonState) -> Option<LeftStickAction> {
    use uinput_sys::{KEY_A, KEY_D, KEY_S, KEY_W};
    use ButtonState::{Press, Release};
    use LeftStickAction::{X, Y};
    use StickAxisState::{High, Low, Neutral};

    Some(match (key, state) {
        (KEY_W, Press /*| Repeat*/) => Y(Low),
        (KEY_S, Press /*| Repeat*/) => Y(High),
        (KEY_W | KEY_S, Release) => Y(Neutral),

        (KEY_A, Press /*| Repeat*/) => X(Low),
        (KEY_D, Press /*| Repeat*/) => X(High),
        (KEY_A | KEY_D, Release) => X(Neutral),

        _ => return None,
    })
}

fn get_time() -> timeval {
    unsafe {
        let mut time = MaybeUninit::uninit();
        assert_eq!(
            libc::gettimeofday(time.as_mut_ptr(), std::ptr::null_mut()),
            0
        );
        time.assume_init()
    }
}

impl GamepadFd {
    pub fn new(dev_name: &[u8]) -> Option<Self> {
        assert!(dev_name.len() <= UINPUT_MAX_NAME_SIZE as usize);
        let fd = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .unwrap()
            .into_raw_fd();
        unsafe {
            // Register A, B, X and Y buttons
            assert!(ui_set_evbit(fd, EV_KEY) >= 0);
            for btn in [BTN_A, BTN_B, BTN_X, BTN_Y] {
                assert!(ui_set_keybit(fd, btn) >= 0);
            }
            // Register left thumbstick
            assert!(ui_set_evbit(fd, EV_ABS) >= 0);
            for abs in [ABS_X, ABS_Y] {
                assert!(ui_set_absbit(fd, abs) >= 0);
            }
        }

        // Create user device
        let uidev = {
            let mut name = [0; UINPUT_MAX_NAME_SIZE as _];
            for (i, c) in dev_name.iter().enumerate() {
                name[i] = *c as i8;
            }
            let mut absmax = [0; ABS_CNT as _];
            let mut absmin = [0; ABS_CNT as _];
            let mut absfuzz = [0; ABS_CNT as _];
            let mut absflat = [0; ABS_CNT as _];

            absmax[ABS_X as usize] = 32767;
            absmin[ABS_X as usize] = -32768;
            absfuzz[ABS_X as usize] = 0;
            absflat[ABS_X as usize] = 15;

            absmax[ABS_Y as usize] = 32767;
            absmin[ABS_Y as usize] = -32768;
            absfuzz[ABS_Y as usize] = 0;
            absflat[ABS_Y as usize] = 15;

            absmax[ABS_RX as usize] = 512;
            absmin[ABS_RX as usize] = -512;
            absfuzz[ABS_RX as usize] = 0;
            absflat[ABS_RX as usize] = 16;

            absmax[ABS_RY as usize] = 512;
            absmin[ABS_RY as usize] = -512;
            absfuzz[ABS_RY as usize] = 0;
            absflat[ABS_RY as usize] = 16;

            uinput_user_dev {
                name,
                id: input_id {
                    // BUS_USB
                    bustype: 0x03,
                    // Microsoft
                    vendor: 0x045E,
                    // Xbox 360 Controller
                    product: 0x028e,
                    version: 0,
                },
                absmax,
                absmin,
                absfuzz,
                absflat,
                ff_effects_max: 0,
            }
        };
        write_type(fd, &uidev).unwrap();
        unsafe {
            if ui_dev_create(fd) >= 0 {
                Some(Self {
                    fd,
                    left_x: Default::default(),
                    left_y: Default::default(),
                    // events: Default::default(),
                })
            } else {
                libc::close(fd);
                None
            }
        }
    }

    /// Adds a button action to the queued events
    pub fn button_action(&mut self, button: GamepadButton, state: ButtonState) {
        self.add_event(input_event {
            time: get_time(),
            kind: EV_KEY as _,
            code: button as _,
            value: state as _,
        })
    }

    fn update_stick(&mut self, action: LeftStickAction) {
        let (dim, state) = match action {
            LeftStickAction::X(state) => (&mut self.left_x, state),
            LeftStickAction::Y(state) => (&mut self.left_y, state),
        };
        use StickAxisState::{High, Low, Neutral};
        *dim = match (*dim, state) {
            (High, High) => High,
            (High, Neutral) => Neutral,
            (High, Low) => Neutral,
            (Neutral, High) => High,
            (Neutral, Neutral) => Neutral,
            (Neutral, Low) => Low,
            (Low, High) => Neutral,
            (Low, Neutral) => Neutral,
            (Low, Low) => Low,
        }
    }

    /// Adds a stick action to the queued events
    pub fn stick_action(&mut self, action: LeftStickAction) {
        self.update_stick(action);
        let (code, value) = match action {
            LeftStickAction::X(_) => (ABS_X as _, self.left_x as _),
            LeftStickAction::Y(_) => (ABS_Y as _, self.left_y as _),
        };
        self.add_event(input_event {
            time: get_time(),
            kind: EV_ABS as _,
            code,
            value,
        })
    }

    fn add_event(&mut self, event: input_event) {
        write_type(self.fd, &event).unwrap()
    }

    /// Writes an `EV_SYN` event
    pub fn sync(&mut self) {
        self.add_event(input_event {
            time: get_time(),
            kind: EV_SYN as _,
            code: 0,
            value: 0,
        });
    }
}
