// Copyright 2024 Brian Cook (a.k.a. Coding-Badly)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::windows::ffi::OsStrExt;

/// Windows (UTF-16) string placed on the stack when possible to improve performance.
///
/// [`WindowsString`] provides a convenient fast way to convert from a Rust UTF-8 string to a
/// Windows API UTF-16 NUL terminated string.  It's typically used for string parameters when
/// calling Windows API functions like [`SetComputerNameW`][scn] and [`ReplaceFileW`][rf].
///
/// A [`WindowsString`] can be zero-sized.  When a [`WindowsString`] is zero-sized, a heap buffer is
/// always used.
///
/// Ideally, a [`WindowsString`] is sized so switching to a heap buffer is rarely necessary.  The
/// [grob crate][gc] provides [`WindowsPathString`][wps] when working with paths.
///
/// # Examples
///
/// This example changes the computer name to `TEMPNAME`.
///
/// ```
/// # #[cfg(not(miri))]
/// # mod miri_skip {
/// #
/// use windows::Win32::Foundation::FALSE;
/// use windows::Win32::System::WindowsProgramming::MAX_COMPUTERNAME_LENGTH;
/// use windows::Win32::System::SystemInformation::SetComputerNameW;
///
/// use grob::{AsPCWSTR, WindowsString};
///
/// const BETTER_MAX_COMPUTERNAME_LENGTH: usize = MAX_COMPUTERNAME_LENGTH as usize;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let new_name = WindowsString::<BETTER_MAX_COMPUTERNAME_LENGTH>::new("TEMPNAME")?;
///     let rv = unsafe { SetComputerNameW(new_name.as_param()) };
///     if rv == FALSE {
///         return Err(std::io::Error::last_os_error().into());
///     }
///     Ok(())
/// }
/// # }
/// ```
///
/// [gc]: https://crates.io/crates/grob
/// [scn]: https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-setcomputernamew
/// [rf]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew
/// [wps]: crate::WindowsPathString
///
pub struct WindowsString<const STACK_BUFFER_SIZE: usize> {
    heap: Option<Vec<u16>>,
    stack: MaybeUninit<[u16; STACK_BUFFER_SIZE]>,
}

impl<const STACK_BUFFER_SIZE: usize> WindowsString<STACK_BUFFER_SIZE> {
    /// Create a [`WindowsString`] with space for `STACK_BUFFER_SIZE` characters on the stack.
    ///
    /// # Errors
    ///
    /// If the string contains any embedded NULs an error is returned.
    ///
    /// # Arguments
    ///
    /// * `s` - The [`OsStr`] to convert to a Windows API UTF-16 NUL terminated string.  Anything
    /// that can be converted to an [`OsStr`] reference, including plain ole Rust strings, can be
    /// passed.
    ///
    pub fn new<S>(s: S) -> std::io::Result<Self>
    where
        S: AsRef<OsStr>,
    {
        let mut rv = Self {
            heap: None,
            stack: MaybeUninit::uninit(),
        };
        rv.convert_and_store(s.as_ref())?;
        Ok(rv)
    }
    /// Return a pointer to the converted Windows API UTF-16 NUL terminated string.
    ///
    /// The return value can be used as-is for Windows API calls defined in the [windows-sys][ws]
    /// crate.
    ///
    /// [ws]: https://crates.io/crates/windows-sys
    ///
    pub fn as_wide(&self) -> *const u16 {
        if self.heap.is_some() {
            unsafe { self.heap.as_ref().map(|v| v.as_ptr()).unwrap_unchecked() }
        } else {
            self.stack.as_ptr() as *const u16
        }
    }

    fn convert_and_store(&mut self, s: &OsStr) -> std::io::Result<()> {
        if s.len() + 1 > STACK_BUFFER_SIZE {
            return self.use_heap(s);
        }
        self.use_stack(s)
    }

    fn use_heap(&mut self, s: &OsStr) -> std::io::Result<()> {
        let mut capacity = s.len() + 1;
        loop {
            let mut buffer = Vec::with_capacity(capacity);
            capacity = buffer.capacity(); // rmv?
            let mut encoder = s.encode_wide();
            let mut p = buffer.as_mut_ptr() as *mut u16;
            let base = p as *const u16;
            let mut finished = false;
            for _ in 0..capacity {
                if let Some(c) = encoder.next() {
                    #[cfg(not(feature = "skip_null_check"))]
                    {
                        if c == 0 {
                            return Err(Self::no_nuls());
                        }
                    }
                    unsafe { *p = c };
                    p = unsafe { p.add(1) };
                } else {
                    unsafe { *p = 0 };
                    finished = true;
                    let stored = unsafe { p.offset_from(base) } + 1;
                    unsafe { buffer.set_len(stored as usize) };
                    self.heap = Some(buffer);
                    break;
                }
            }
            if finished {
                break;
            }
            // Note: This point was never reached during testing.
            capacity *= 2;
        }
        Ok(())
    }

    fn use_stack(&mut self, s: &OsStr) -> std::io::Result<()> {
        let mut encoder = s.encode_wide();
        let mut p = self.stack.as_mut_ptr() as *mut u16;
        let mut finished = false;
        for _ in 0..STACK_BUFFER_SIZE {
            if let Some(c) = encoder.next() {
                // https://github.com/rust-lang/rust/blob/6f435eb0eb2926cdb6640b3382b9e3e21ef05f07/library/std/src/sys/pal/windows/mod.rs#L184
                #[cfg(not(feature = "skip_null_check"))]
                {
                    if c == 0 {
                        return Err(Self::no_nuls());
                    }
                }
                unsafe { *p = c };
                p = unsafe { p.add(1) };
            } else {
                unsafe { *p = 0 };
                finished = true;
                break;
            }
        }
        if !finished {
            // Note: This point was never reached during testing.
            return self.use_heap(s);
        }
        Ok(())
    }

    #[cfg(not(feature = "skip_null_check"))]
    fn no_nuls() -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "strings passed to WinAPI cannot contain NULs",
        )
    }
}
