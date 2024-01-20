use std::os::{fd::IntoRawFd, unix::fs::OpenOptionsExt};

use libc::{pollfd, POLLIN};
use uinput_sys::input_event;

#[derive(Clone, Copy, Debug)]
pub struct KeyboardFd(i32);

impl KeyboardFd {
    /// Use [`libc::poll`] to block until there is an event ready
    pub fn wait_for_event(&self) {
        let mut fds = [pollfd {
            fd: self.0,
            events: POLLIN,
            revents: 0,
        }];
        unsafe {
            assert!(libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) > 0);
        }
        assert_eq!(fds[0].revents, POLLIN);
    }

    /// Read an [`input_event`] from the device, if any are available.
    pub fn read_event(&self) -> Option<input_event> {
        unsafe { crate::util::read_type(self.0).ok() }
    }
}

pub fn get_keyboard(path: &str) -> Option<KeyboardFd> {
    let keyboard = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?
        .into_raw_fd();
    // TODO: verify this is a keyboard
    Some(KeyboardFd(keyboard))
}
