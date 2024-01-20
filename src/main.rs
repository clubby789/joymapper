mod gamepad;
mod keyboard;
mod util;

use gamepad::GamepadFd;
use keyboard::KeyboardFd;
use uinput_sys::input_event;

fn event_loop(keyboard: KeyboardFd, mut gamepad: GamepadFd) {
    loop {
        keyboard.wait_for_event();
        while let Some(ev) = keyboard.read_event() {
            handle_press(&ev, &mut gamepad);
        }
        gamepad.sync();
    }
}

fn handle_press(event: &input_event, gamepad: &mut GamepadFd) {
    use uinput_sys::*;
    if event.kind != EV_KEY as _ {
        return;
    }
    assert!(matches!(event.value, 0..=2));
    if let Some(button) = gamepad::map_button(event.code as i32) {
        gamepad.button_action(button, event.value.into());
    } else if let Some(stick) = gamepad::map_stick(event.code as i32, event.value.into()) {
        gamepad.stick_action(stick);
    }
}

fn main() {
    let kb = keyboard::get_keyboard("/dev/input/event3").unwrap();
    let gamepad = gamepad::GamepadFd::new("Fake Gamepad".as_bytes()).unwrap();
    event_loop(kb, gamepad);
}
