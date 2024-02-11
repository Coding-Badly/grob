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

use std::ffi::OsString;

use windows::core::PWSTR;

use crate::buffer::StackBuffer;
use crate::strategy::{
    GrowForSmallBinary, GrowForStaticText, GrowForStoredIsReturned, GrowToNearestQuarterKibi,
};
use crate::traits::{GrowStrategy, RawToInternal, ToResult, WriteBuffer};
use crate::win::{CAPACITY_FOR_NAMES, CAPACITY_FOR_PATHS};
use crate::{Argument, FrozenBuffer, GrowableBuffer};

/// Generic growable buffer loop.
///
/// This generic function implements the call-operating-system-grow-buffer loop.  It is not meant to
/// be used directly.
///
pub fn winapi_generic<FT, IT, W, WR, F, U>(
    mut growable_buffer: GrowableBuffer<FT, IT>,
    mut api_wrapper: W,
    mut finalize: F,
) -> Result<U, std::io::Error>
where
    IT: RawToInternal,
    IT: Copy,
    WR: ToResult,
    W: FnMut(&mut Argument<IT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    loop {
        let mut argument = growable_buffer.argument();
        let rv = api_wrapper(&mut argument);
        let fill_buffer_action = rv.to_result(&mut argument)?;
        if argument.apply(fill_buffer_action) {
            break;
        }
    }
    finalize(growable_buffer.freeze())
}

/// Generic growable buffer loop for binary data (the result datatype is implied).
///
/// This generic function is the common code for [`winapi_large_binary`] and
/// [`winapi_small_binary`].  It is not meant to be used directly.
///
pub fn winapi_binary<FT, W, WR, F, U>(
    initial_buffer: &mut dyn WriteBuffer,
    grow_strategy: &dyn GrowStrategy,
    api_wrapper: W,
    finalize: F,
) -> Result<U, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<*mut FT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    let growable_buffer = GrowableBuffer::<FT, *mut FT>::new(initial_buffer, grow_strategy);
    winapi_generic(growable_buffer, api_wrapper, finalize)
}

/// Generic wrapper function for a Windows API call that returns binary data and needs a relatively small buffer
///
/// # Arguments
///
/// * `api_wrapper` - The Windows API call is made inside this closure.  The argument for the call
///     is provided.  The return value from the closure is either an [`RvIsError`][e] or an
///     [`RvIsSize`][s].
///
/// * `finalize` - If the Windows API call is successful, this closure is passed a [`FrozenBuffer`]
///     that allows access to the data.
///
/// # Returns
///
/// The return value from `winapi_small_binary` is...
///
/// * `Ok( /* success value */ )` when the operating system call and the `finalize` closure return
///     success where `success value` is the value returned from the `finalize` closure
///
/// * `Err(`[`std::io::Error`]`)` when the operating system call fails or the `finalize` closure
///     returns an error
///
/// # Examples
///
/// This example returns the maximum processor count for the first processor group.
///
/// ```
/// use grob::{winapi_small_binary, RvIsError};
///
/// use windows::Win32::System::SystemInformation::{GetLogicalProcessorInformationEx, RelationGroup};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mpc = winapi_small_binary(
///         |argument| {
///             RvIsError::new(unsafe {
///                 GetLogicalProcessorInformationEx(
///                     RelationGroup,
///                     Some(argument.pointer()),
///                     argument.size(),
///                 )
///             })
///         },
///         |frozen_buffer| {
///             if let Some(p) = frozen_buffer.pointer() {
///                 let r = unsafe { (*p).Relationship };
///                 let mpc = unsafe { (*p).Anonymous.Group.GroupInfo[0].MaximumProcessorCount };
///                 println!("Relationship = {:?}", r); // Has to be RelationGroup
///                 println!("Size = {:?}", unsafe { (*p).Size });
///                 println!("MaximumGroupCount = {:?}", unsafe {
///                     (*p).Anonymous.Group.MaximumGroupCount
///                 });
///                 println!("ActiveGroupCount = {:?}", unsafe {
///                     (*p).Anonymous.Group.ActiveGroupCount
///                 });
///                 println!("ActiveProcessorCount = {:?}", unsafe {
///                     (*p).Anonymous.Group.GroupInfo[0].ActiveProcessorCount
///                 });
///                 println!("MaximumProcessorCount = {:?}", mpc);
///                 println!();
///                 Ok(Some(mpc))
///             } else {
///                 Ok(None)
///             }
///         },
///     )?;
///
///     if let Some(mpc) = mpc {
///         println!("Success!  The maximum processor count is {}.", mpc);
///     }
///     Ok(())
/// }
/// ```
///
/// [e]: crate::RvIsError
/// [s]: crate::RvIsSize
/// [gaa]: https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses
///
pub fn winapi_small_binary<FT, W, WR, F, U>(
    api_wrapper: W,
    finalize: F,
) -> Result<U, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<*mut FT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    let mut initial_buffer = StackBuffer::<1024>::new();
    let grow_strategy = GrowForSmallBinary::new();
    winapi_binary(&mut initial_buffer, &grow_strategy, api_wrapper, finalize)
}

/// Generic wrapper function for a Windows API call that returns binary data and needs a relatively large buffer.
///
/// # Arguments
///
/// * `api_wrapper` - The Windows API call is made inside this closure.  The argument for the call
///     is provided.  The return value from the closure is either an [`RvIsError`][e] or an
///     [`RvIsSize`][s].
///
/// * `finalize` - If the Windows API call is successful, this closure is passed a [`FrozenBuffer`]
///     that allows access to the data.
///
/// # Returns
///
/// The return value from `winapi_large_binary` is...
///
/// * `Ok( /* success value */ )` when the operating system call and the `finalize` closure return
///     success where `success value` is the value returned from the `finalize` closure
///
/// * `Err(`[`std::io::Error`]`)` when the operating system call fails or the `finalize` closure
///     returns an error
///
/// # Examples
///
/// This example builds a list of the friendly names of the network adapters returned from
/// [`GetAdaptersAddresses`][gaa] then prints those names.
///
/// ```
/// use windows::Win32::{
///     NetworkManagement::IpHelper::{GetAdaptersAddresses, GET_ADAPTERS_ADDRESSES_FLAGS},
///     Networking::WinSock::AF_UNSPEC,
/// };
///
/// use grob::{winapi_large_binary, RvIsError};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut names = winapi_large_binary(
///         |argument| {
///             RvIsError::new(unsafe {
///                 GetAdaptersAddresses(
///                     AF_UNSPEC.0 as u32,
///                     GET_ADAPTERS_ADDRESSES_FLAGS(0),
///                     None,
///                     Some(argument.pointer()),
///                     argument.size(),
///                 )
///             })
///         },
///         |frozen_buffer| {
///             let mut rv = Vec::new();
///             if let Some(mut p) = frozen_buffer.pointer() {
///                 while p != std::ptr::null() {
///                     rv.push(format!("{}", unsafe { (*p).FriendlyName.display() } ));
///                     p = unsafe { (*p).Next };
///                 }
///             }
///             Ok(rv)
///         },
///     )?;
///     names.sort();
///     println!("Names...");
///     for name in names.into_iter() {
///         println!("  {}", name);
///     }
///     Ok(())
/// }
/// ```
///
/// [e]: crate::RvIsError
/// [s]: crate::RvIsSize
/// [gaa]: https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses
///
pub fn winapi_large_binary<FT, W, WR, F, U>(
    api_wrapper: W,
    finalize: F,
) -> Result<U, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<*mut FT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    let mut initial_buffer = StackBuffer::<65536>::new();
    let grow_strategy = GrowToNearestQuarterKibi::new();
    winapi_binary(&mut initial_buffer, &grow_strategy, api_wrapper, finalize)
}

/// Generic wrapper for a Windows API call that returns a file system path.
///
/// # Arguments
///
/// * `api_wrapper` - The Windows API call is made inside this closure.  The argument for the call
///     is provided.  The return value from the closure is an [`RvIsSize`][s].
///
/// [s]: crate::RvIsSize
///
/// # Returns
///
/// The return value from `winapi_path_buf` is...
///
/// * `Ok(`[`PathBuf`][pb]`)` when the operating system call returns success
///
/// * `Err(`[`std::io::Error`]`)` when the operating system call fails
///
/// [pb]: std::path::PathBuf
///
/// # Examples
///
/// This example calls [`GetModuleFileNameW`][gm] to get the full path to the running program then
/// prints that path.
///
/// [gm]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/LibraryLoader/fn.GetModuleFileNameW.html
///
/// ```
/// use windows::Win32::Foundation::HMODULE;
/// use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
///
/// use grob::{winapi_path_buf, RvIsSize};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let path = winapi_path_buf(|argument| {
///         RvIsSize::new(unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) })
///     })?;
///     println!("GetModuleFileNameW returned {}", path.display());
///     Ok(())
/// }
/// ```
///
/// This example prints the full path to the Windows system directory ([`GetSystemWindowsDirectoryW`][sd]).
///
/// [sd]: https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemwindowsdirectoryw
///
/// ```
/// use std::path::PathBuf;
///
/// use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
///
/// use grob::{winapi_path_buf, RvIsSize};
///
/// fn get_windows_system_dir() -> Result<PathBuf, std::io::Error> {
///     winapi_path_buf(|argument| {
///         RvIsSize::new(unsafe { GetSystemWindowsDirectoryW(Some(argument.as_mut_slice())) })
///     })
/// }
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     println!("{:?}", get_windows_system_dir());
///     Ok(())
/// }
/// ```
///
pub fn winapi_path_buf<W, WR>(api_wrapper: W) -> Result<std::path::PathBuf, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<PWSTR>) -> WR,
{
    let mut initial_buffer = StackBuffer::<CAPACITY_FOR_PATHS>::new();
    const CFP: u64 = CAPACITY_FOR_PATHS as u64;
    let grow_strategy = GrowForStoredIsReturned::<CFP>::new();
    let growable_buffer = GrowableBuffer::<u16, PWSTR>::new(&mut initial_buffer, &grow_strategy);
    winapi_generic(growable_buffer, api_wrapper, |frozen_buffer| {
        Ok(frozen_buffer.to_path_buf().unwrap_or_default())
    })
}

/// Generic wrapper for a Windows API call that returns a text string like the computer or user name.
///
/// # Arguments
///
/// * `lossy_ok` - Is returning a lossy string okay?  See [`to_string`][ts] for details.
/// * `api_wrapper` - The Windows API call is made inside this closure.  The argument for the call
///     is provided.  The return value from the closure is either an [`RvIsError`][e] or an
///     [`RvIsSize`][s].
///
/// [e]: crate::RvIsError
/// [s]: crate::RvIsSize
/// [ts]: crate::FrozenBuffer::to_string
///
/// # Returns
///
/// The return value from `winapi_string` is...
///
/// * `Ok(Ok(`[`String`]`))` when the operating system call returns success and...
///     * Either `lossy_ok` is `true`
///     * Or `lossy_ok` is `false` and the data returned from the operating system can be
///         converted to a UTF-8 string without problems
///
/// * `Ok(Err(`[`OsString`]`))` when the operating system call returns success and `lossy_ok` is
///     `false` and the data returned from the operating system _cannot_ be converted to a valid
///     UTF-8 string
///
/// * `Err(Err(`[`std::io::Error`]`))` when the operating system call fails
///
/// # Examples
///
/// The following example prints the user name returned from [`GetUserNameW`][un].  [`unwrap`][uw]
/// is used because `lossy_ok` is `true`.  Any invalid Unicode sequences are replaced so either
/// `Ok(Ok(`[`String`]`))` or `Err(Err(`[`std::io::Error`]`))` is returned;
/// `Ok(Err(`[`OsString`]`))` is never returned when `lossy_ok` is `true`.  See
/// [`to_string_lossy`][sl] for details.
///
/// [un]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getusernamew
/// [uw]: std::option::Option::unwrap
/// [sl]: std::ffi::OsStr::to_string_lossy
///
/// ```
/// use windows::Win32::System::WindowsProgramming::GetUserNameW;
///
/// use grob::{winapi_string, RvIsError};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let username = winapi_string(true, |argument| {
///         RvIsError::new(unsafe { GetUserNameW(argument.pointer(), argument.size()) })
///     })?
///     .unwrap();
///     println!("GetUserNameW returned {}", username);
///     Ok(())
/// }
/// ```
///
pub fn winapi_string<W, WR>(
    lossy_ok: bool,
    api_wrapper: W,
) -> Result<Result<String, OsString>, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<PWSTR>) -> WR,
{
    let mut initial_buffer = StackBuffer::<CAPACITY_FOR_NAMES>::new();
    let grow_strategy = GrowForStaticText::new();
    let growable_buffer = GrowableBuffer::<u16, PWSTR>::new(&mut initial_buffer, &grow_strategy);
    winapi_generic(growable_buffer, api_wrapper, |frozen_buffer| {
        Ok(frozen_buffer.to_string(lossy_ok))
    })
}
