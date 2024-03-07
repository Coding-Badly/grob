// Copyright 2023 Brian Cook (a.k.a. Coding-Badly)
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

use std::ffi::{OsStr, OsString};
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::slice::{from_raw_parts, from_raw_parts_mut};

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    GetLastError, SetLastError, BOOL, ERROR_BUFFER_OVERFLOW, ERROR_INSUFFICIENT_BUFFER,
    ERROR_NO_DATA, MAX_PATH, NO_ERROR, TRUE, WIN32_ERROR,
};
use windows::Win32::NetworkManagement::NetManagement::UNLEN;

use crate::base::{FillBufferAction, FillBufferResult};
use crate::buffer::os::ALIGNMENT;
use crate::traits::{NeededSize, RawToInternal, ToResult};
use crate::winstr::WindowsString;
use crate::{Argument, FrozenBuffer};

const BETTER_MAX_PATH: usize = MAX_PATH as usize;

/// Size of [`WCHAR`][wc] / [`u16`] (two bytes) cast as a [`u32`].
///
/// The value is cast to [`u32`] to make it more convenient when working with buffer capacities.
///
/// [gc]: https://crates.io/crates/grob
/// [wc]: https://learn.microsoft.com/en-us/windows/win32/extensible-storage-engine/wchar
///
pub const SIZE_OF_WCHAR: u32 = size_of::<u16>() as u32;

/// A good starting buffer capacity, in bytes, for Windows API calls that return the name of something.
///
/// The value is based on [`UNLEN`].  According to the Windows API documentation this value works
/// as-is for some operating system calls like [`GetUserNameW`][1].
///
/// [`winapi_string`][2] uses this value for the initial stack buffer capacity.
///
/// [1]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getusernamew
/// [2]: crate::generic::winapi_string
///
pub const CAPACITY_FOR_NAMES: usize = ((UNLEN + 1) as usize * SIZE_OF_WCHAR as usize) + ALIGNMENT;

/// A good starting buffer capacity, in bytes, for Windows API calls that return a file system path.
///
/// The value is based on [`MAX_PATH`].  Windows has support for arbitrarily long paths so this
/// value is only useful as a starting buffer capacity.  [`GetModuleFileNameW`][4] is an example API
/// call where this value is useful.
///
/// [`winapi_path_buf`][3] uses this value for the initial stack buffer capacity.
///
/// [3]: crate::generic::winapi_path_buf
/// [4]: https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulefilenamew
///
pub const CAPACITY_FOR_PATHS: usize =
    (BETTER_MAX_PATH as usize * SIZE_OF_WCHAR as usize) + ALIGNMENT;

impl<'gb> Argument<'gb, PWSTR> {
    /// Provides access to the buffer through a writable slice of [`u16`]
    ///
    /// Some Windows API calls, like [`GetModuleFileNameW`][1], take a `&mut [u16]`.  This method
    /// provides that argument.
    ///
    /// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/LibraryLoader/fn.GetModuleFileNameW.html
    ///
    pub fn as_mut_slice(&mut self) -> &mut [u16] {
        let rv = unsafe { from_raw_parts_mut(self.pointer.0, self.size as usize) };
        unsafe { SetLastError(NO_ERROR) };
        rv
    }
}

/// Wrapper for the return value from a Windows API call that returns an error code.
///
/// The primary purpose of [`RvIsError`] is to convert a [`BOOL`] or [`u32`] (ULONG) Windows API
/// return value into a [`FillBufferResult`].  The [`FillBufferResult`] is either
/// Ok([`FillBufferAction`]) or an operating system error (Err([`std::io::Error`])) that is not
/// handled by the [grob crate][gc].
///
/// # Examples
///
/// [`GetAdaptersAddresses`][1] is a good example for [`RvIsError`].  A complete example is
/// available on [GitHub][2].
///
/// ``` ignore
/// // Make the API call indicating what the return value means
/// let rv = RvIsError::new(unsafe {
///     GetAdaptersAddresses(
///         AF_UNSPEC.0 as u32,
///         GET_ADAPTERS_ADDRESSES_FLAGS(0),
///         None,
///         Some(argument.pointer()),
///         argument.size(),
///     )
/// });
///
/// // Convert the return value to an action
/// let fill_buffer_action = rv.to_result(&mut argument)?;
/// ```
///
/// [`GetLogicalProcessorInformationEx`][3] is also a good example for [`RvIsError`].  A complete
/// example is available on [GitHub][4].
///
/// [gc]: https://crates.io/crates/grob
/// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/NetworkManagement/IpHelper/fn.GetAdaptersAddresses.html
/// [2]: https://github.com/Coding-Badly/grob/blob/main/grob/examples/adapters-addresses-full.rs
/// [3]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/SystemInformation/fn.GetLogicalProcessorInformationEx.html
/// [4]: https://github.com/Coding-Badly/grob/blob/main/grob/examples/processor-full.rs
///
#[derive(Debug)]
pub struct RvIsError(WIN32_ERROR);

impl RvIsError {
    pub fn new<T>(value: T) -> Self
    where
        T: Into<Self>,
    {
        value.into()
    }
}

impl ToResult for RvIsError {
    /// Determines what should happen based on the value returned from the operating system and the
    /// [`Argument`] state.
    ///
    /// If the return value is a [`u32`], like [`GetAdaptersAddresses`][2], the value is used as-is.
    ///
    /// For operating system functions that return a [`BOOL`], like
    /// [`GetLogicalProcessorInformationEx`][3], the error code [`NO_ERROR`] is used when [`TRUE`]
    /// is returned.  The return value from [`GetLastError`] is used when [`TRUE`] is not returned.
    ///
    /// Operating system error codes are translated by this method to...
    ///
    /// | Error Code                    | [`FillBufferResult`]             |
    /// | ----------------------------- | -------------------------------- |
    /// | [`NO_ERROR`]                  | Ok([`FillBufferAction::Commit`]) |
    /// | [`ERROR_INSUFFICIENT_BUFFER`] | Ok([`FillBufferAction::Grow`])   |
    /// | [`ERROR_BUFFER_OVERFLOW`]     | Ok([`FillBufferAction::Grow`])   |
    /// | [`ERROR_NO_DATA`]             | Ok([`FillBufferAction::NoData`]) |
    /// | all other values              | Err(/\*osecctsie\*/)             |
    ///
    /// Where /\*osecctsie\*/ is the operating system error code converted to a [`std::io::Error`]
    /// by calling [`from_raw_os_error`][1].
    ///
    /// [1]: std::io::Error::from_raw_os_error
    /// [2]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/NetworkManagement/IpHelper/fn.GetAdaptersAddresses.html
    /// [3]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/SystemInformation/fn.GetLogicalProcessorInformationEx.html
    ///
    fn to_result(&self, needed_size: &mut dyn NeededSize) -> FillBufferResult {
        let rv = match self.0 {
            NO_ERROR => Ok(FillBufferAction::Commit),
            ERROR_INSUFFICIENT_BUFFER => Ok(FillBufferAction::Grow),
            ERROR_BUFFER_OVERFLOW => Ok(FillBufferAction::Grow),
            ERROR_NO_DATA => Ok(FillBufferAction::NoData),
            c => Err(std::io::Error::from_raw_os_error(c.0 as i32)),
        };
        if rv.is_ok() && needed_size.needed_size() == 0 {
            Ok(FillBufferAction::NoData)
        } else {
            rv
        }
    }
}

impl From<BOOL> for RvIsError {
    fn from(value: BOOL) -> Self {
        if value == TRUE {
            Self(NO_ERROR)
        } else {
            Self(unsafe { GetLastError() })
        }
    }
}

impl From<u32> for RvIsError {
    fn from(value: u32) -> Self {
        Self(WIN32_ERROR(value))
    }
}

/// Wrapper for the return value from a Windows API call that returns the number of elements stored
///
/// The primary purpose of [`RvIsSize`] is to convert the number of elements stored and the value
/// returned from [`GetLastError`] into a [`FillBufferResult`].  The [`FillBufferResult`] is either
/// Ok([`FillBufferAction`]) or an operating system error (Err([`std::io::Error`])) that is not
/// handled by the [grob crate][gc].
///
/// # Examples
///
/// [`GetModuleFileNameW`][1] is a good example for [`RvIsSize`].  A complete example is
/// available on [GitHub][2].
///
/// ``` ignore
/// let mut argument = growable_buffer.argument();
/// let rv = unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) };
/// let rv: RvIsSize = rv.into();
/// let result = rv.to_result(&mut argument);
/// match result? {
///     FillBufferAction::Commit => {
///         argument.commit();
///         break;
///     }
///     FillBufferAction::Grow => {
///         argument.grow();
///     }
///     FillBufferAction::NoData => {
///         argument.commit_no_data();
///         break;
///     }
/// }
/// ```
///
/// [`GetSystemWindowsDirectoryW`][3] is also a good example for [`RvIsError`].  A complete example
/// is available on [GitHub][4].
///
/// [gc]: https://crates.io/crates/grob
/// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/LibraryLoader/fn.GetModuleFileNameW.html
/// [2]: https://github.com/Coding-Badly/grob/blob/main/grob/examples/module-filename-full.rs
/// [3]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/SystemInformation/fn.GetSystemWindowsDirectoryW.html
/// [4]: https://github.com/Coding-Badly/grob/blob/main/grob/examples/version-info-generic.rs
///
#[derive(Debug)]
pub struct RvIsSize(u32, WIN32_ERROR);

impl RvIsSize {
    pub fn new<T>(value: T) -> Self
    where
        T: Into<Self>,
    {
        value.into()
    }
}

impl ToResult for RvIsSize {
    /// Determines what should happen based on the value returned from the operating system and the
    /// [`Argument`] state.
    ///
    /// The return value from the operating system is expected to be the number of elements stored.
    ///
    /// The return value from [`GetLastError`] is captured when [`RvIsSize`] is created.  It's
    /// important to "clear" the error value by calling `SetLastError(NO_ERROR)` just before calling
    /// the Windows API function then creating an [`RvIsSize`] right after calling the Windows API
    /// function.  This crate handles all of that when used as documented.
    ///
    /// The various states are translated as...
    ///
    /// | Return Value       | Capacity | [`GetLastError`]              | [`FillBufferResult`]             |
    /// | ------------------ | -------- | ----------------------------- | -------------------------------- |
    /// | zero               | n/a      | [`NO_ERROR`]                  | Ok([`FillBufferAction::NoData`]) |
    /// | zero               | zero     | n/a                           | Ok([`FillBufferAction::Grow`])   |
    /// | zero               | not zero | all other values              | Err(/\*osecctsie\*/)             |
    /// | > 0 && < Capacity  | > 0      | n/a                           | Ok([`FillBufferAction::Commit`]) |
    /// | > 0 && == Capacity | > 0      | [`ERROR_INSUFFICIENT_BUFFER`] | Ok([`FillBufferAction::Grow`])   |
    ///
    /// Where /\*osecctsie\*/ is the operating system error code converted to a [`std::io::Error`]
    /// by calling [`from_raw_os_error`][1].
    ///
    /// [1]: std::io::Error::from_raw_os_error
    ///
    fn to_result(&self, needed_size: &mut dyn NeededSize) -> FillBufferResult {
        let ns = needed_size.needed_size();
        // Either an error or success with nothing stored
        if self.0 == 0 {
            // Success with nothing stored
            if self.1 == NO_ERROR {
                Ok(FillBufferAction::NoData)
            // The buffer has no capacity.  Very likely because the caller does not want to use a
            // stack buffer.  The expectation is that the GrowStrategy will have a reasonable
            // minimum capacity so we'll just indicate something more than zero.
            } else if ns == 0 {
                needed_size.set_needed_size(1);
                Ok(FillBufferAction::Grow)
            // Error
            } else {
                Err(std::io::Error::from_raw_os_error(self.1 .0 as i32))
            }
        // Buffer was big enough.  self.1 is presumed to be NO_ERROR.
        } else if self.0 < ns {
            needed_size.set_needed_size(self.0);
            Ok(FillBufferAction::Commit)
        // Buffer does not have space for the terminator.
        } else if self.1 == ERROR_INSUFFICIENT_BUFFER {
            needed_size.set_needed_size(self.0.saturating_mul(2));
            Ok(FillBufferAction::Grow)
        // At this point the API function returned precisely the buffer capacity and set the last
        // error to something other than ERROR_INSUFFICIENT_BUFFER.  Or, the API function returned a
        // value greater than the capacity.  Those are both undocument behaviours.
        } else {
            unreachable!()
        }
    }
}

impl From<u32> for RvIsSize {
    fn from(value: u32) -> Self {
        let gle = unsafe { GetLastError() };
        Self(value, gle)
    }
}

impl RawToInternal for PWSTR {
    fn capacity_to_size(value: u32) -> u32 {
        // The size is specified in WCHARs.
        value / crate::SIZE_OF_WCHAR
    }
    fn convert_pointer(value: *mut u8) -> PWSTR {
        PWSTR(value as *mut u16)
    }
    fn size_to_capacity(value: u32) -> u32 {
        // The size is specified in WCHARs.
        value.saturating_mul(crate::SIZE_OF_WCHAR)
    }
}

impl<'sb> FrozenBuffer<'sb, u16> {
    /// Convert the data in the buffer to a [`PathBuf`].
    ///
    /// This method passes the return value from [`to_os_string`](FrozenBuffer::to_os_string) to
    /// `PathBuf::from`.
    ///
    /// If the call to [`read_buffer`](FrozenBuffer::read_buffer) returns a [`null`](std::ptr::null)
    /// pointer or zero elements were stored in the buffer then [`None`] is returned from this
    /// method.
    ///
    /// A `NULL` terminator, if present, is not included in the returned [`PathBuf`].
    ///
    pub fn to_path_buf(&self) -> Option<PathBuf> {
        self.to_os_string().map(PathBuf::from)
    }
    /// Convert the data in the buffer to an [`OsString`].
    ///
    /// If the call to [`read_buffer`](FrozenBuffer::read_buffer) returns a [`null`](std::ptr::null)
    /// pointer or zero elements were stored in the buffer then [`None`] is returned from this
    /// method.
    ///
    /// A `NULL` terminator, if present, is not included in the returned [`OsString`].
    ///
    pub fn to_os_string(&self) -> Option<OsString> {
        let (p, s) = self.read_buffer();
        if s == 0 {
            return None;
        }
        assert!(s > 0);
        if let Some(p) = p {
            let v = unsafe { from_raw_parts(p, s as usize) };
            // Protected by the "s == 0" check and assert above.
            let last: usize = if *v.last().unwrap() == 0 { s - 1 } else { s }
                .try_into()
                .unwrap();
            Some(OsString::from_wide(&v[..last]))
        } else {
            None
        }
    }
    /// Try converting the data in the buffer to a [`String`].
    ///
    /// If `lossy_ok` is [`true`] then the call cannot fail.  `Ok(possibly_lossy_string)` is always
    /// returned.  Any invalid characters are replaced according to the
    /// [`to_string_lossy`](std::ffi::OsStr::to_string_lossy) documentation.
    ///
    /// If `lossy_ok` is [`false`] and the buffer contains a valid UTF-8 string then
    /// `Ok(converted_string)` is returned.
    ///
    /// If `lossy_ok` is [`false`] and the buffer contains invalid UTF-8 characters then
    /// `Err(raw_os_string)` is returned where `raw_os_string` is an [`OsString`] returned from
    /// [`to_os_string`](FrozenBuffer::to_os_string)
    ///
    /// A `NULL` terminator, if present, is not included in the returned value.
    ///
    /// If the call to [`to_os_string`](FrozenBuffer::to_os_string) returns [`None`] then a zero
    /// length / blank string is returned.
    ///
    pub fn to_string(&self, lossy_ok: bool) -> Result<String, OsString> {
        match self.to_os_string() {
            Some(s) => {
                if lossy_ok {
                    Ok(s.to_string_lossy().to_string())
                } else {
                    s.into_string()
                }
            }
            None => Ok(String::new()),
        }
    }
}

pub trait AsPCWSTR {
    fn as_param(&self) -> PCWSTR;
}

/// Windows (UTF-16) string placed on the stack when possible to improve performance sized for
/// paths.
///
/// [`WindowsPathString`] provides a convenient fast way to convert from a Rust UTF-8 string to a
/// Windows API UTF-16 NUL terminated string.  It's typically used for path parameters when calling
/// Windows API functions like [`ReplaceFileW`][rf].
///
/// # Examples
///
/// This example creates a file using functions from the Rust Standard Library then deletes that
/// file using the Windows API [`DeleteFileW`][df] function.
///
/// ```
/// # #[cfg(not(miri))]
/// # mod miri_skip {
/// #
/// use std::fs::{canonicalize, File};
/// use std::io::Write;
///
/// use windows::Win32::{Foundation::TRUE, Storage::FileSystem::DeleteFileW};
///
/// use grob::{AsPCWSTR, WindowsPathString};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let working_dir = canonicalize(".")?;
///     let target_path = working_dir.join("delete-me.tmp");
///
///     let mut output = File::create(&target_path)?;
///     write!(output, "Please delete this file.")?;
///     drop(output);
///
///     let rv = unsafe { DeleteFileW(WindowsPathString::new(&target_path)?.as_param()) };
///     if rv == TRUE {
///         println!("{} successfully deleted.", target_path.display());
///     } else {
///         let loe = std::io::Error::last_os_error();
///         println!("DeleteFileW failed.  The error is...\n  {:?}.", loe);
///     }
///
///     Ok(())
/// }
/// # }
/// ```
///
/// [rf]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew
/// [df]: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-deletefilew
///
pub struct WindowsPathString {}

impl WindowsPathString {
    /// Create a [`WindowsString`] with space for [`MAX_PATH`] characters on the stack.
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
    pub fn new<S>(s: S) -> std::io::Result<WindowsString<BETTER_MAX_PATH>>
    where
        S: AsRef<OsStr>,
    {
        WindowsString::new(s)
    }
}

impl<const STACK_BUFFER_SIZE: usize> AsPCWSTR for WindowsString<STACK_BUFFER_SIZE> {
    /// Return a pointer to the converted Windows API UTF-16 NUL terminated string wrapped in a [`PCWSTR`].
    ///
    /// The return value can be used as-is for Windows API calls defined in the [windows][ws] crate.
    ///
    /// [ws]: https://crates.io/crates/windows
    ///
    fn as_param(&self) -> PCWSTR {
        PCWSTR(self.as_wide())
    }
}
