use std::mem::MaybeUninit;

pub fn write_type<T: Sized>(fd: i32, val: &T) -> Result<(), ()> {
    let size = std::mem::size_of::<T>();
    let ret = unsafe { libc::write(fd, val as *const _ as _, size) };
    if ret == size as isize {
        Ok(())
    } else {
        Err(())
    }
}

/// Read a given libc type from a file descriptor.
/// # Safety
/// `fd` must be a file descriptor that produces values of type `T`.
pub unsafe fn read_type<T>(fd: i32) -> Result<T, ()> {
    let size = std::mem::size_of::<T>();
    let mut buf = MaybeUninit::<T>::uninit();
    let ret = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), size) };
    if ret == size as isize {
        unsafe { Ok(buf.assume_init()) }
    } else {
        Err(())
    }
}
