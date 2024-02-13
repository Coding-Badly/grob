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

use crate::base::FillBufferResult;

pub(crate) trait GrowableBufferAsParent {
    fn grow(&mut self, value: u32);
    fn set_final_size(&mut self, value: u32);
}

/// How should the buffer grow?  Small bump?  Double in capacity?
///
/// # Examples
///
/// ```
/// # #[cfg(not(miri))]
/// # mod miri_skip {
/// #
/// use std::path::PathBuf;
///
/// use windows::{
///     core::PWSTR,
///     Win32::Foundation::HMODULE,
///     Win32::System::LibraryLoader::GetModuleFileNameW,
/// };
///
/// use grob::{
///     GrowableBuffer,
///     GrowStrategy,
///     FillBufferAction,
///     RvIsSize,
///     StackBuffer,
///     ToResult,
/// };
///
/// struct GrowExponentially {}
///
/// impl GrowStrategy for GrowExponentially {
///     fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32 {
///         let guess = 1 << tries;
///         if guess < desired_capacity {
///             desired_capacity
///         } else {
///             guess
///         }
///     }
/// }
///
/// struct PrintNextCapacity {
///     wrapped: Box<dyn GrowStrategy>,
/// }
///
/// impl PrintNextCapacity {
///     fn new<GS>(wrapped: GS) -> Self
///     where
///         GS: GrowStrategy + 'static,
///     {
///         let wrapped = Box::new(wrapped);
///         Self {
///             wrapped,
///         }
///     }
/// }
///
/// impl GrowStrategy for PrintNextCapacity {
///     fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32 {
///         let rv = self.wrapped.next_capacity(tries, desired_capacity);
///         println!("next_capacity(tries={}, desired_capacity={}) = {}", tries, desired_capacity, rv);
///         rv
///     }
/// }
///
/// fn get_our_module_filename() -> Result<PathBuf,Box<dyn std::error::Error>> {
///     let mut initial_buffer = StackBuffer::<0>::new();
///     let grow_strategy = PrintNextCapacity::new(GrowExponentially {});
///     let mut growable_buffer = GrowableBuffer::<u16, PWSTR>::new(&mut initial_buffer, &grow_strategy);
///     loop {
///         let mut argument = growable_buffer.argument();
///         let result = RvIsSize::new(unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) })
///             .to_result(&mut argument)?;
///         match result {
///             FillBufferAction::Commit => {
///                 argument.commit();
///                 break;
///             }
///             FillBufferAction::Grow => {
///                 argument.grow();
///             }
///             FillBufferAction::NoData => {
///                 argument.commit_no_data();
///                 break;
///             }
///         }
///     }
///     let frozen_buffer = growable_buffer.freeze();
///     Ok(frozen_buffer.to_path_buf().unwrap())
/// }
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     println!("This program lives at {}.", get_our_module_filename()?.display());
///     Ok(())
/// }
/// # }
/// ```
///
pub trait GrowStrategy {
    /// Returns the next larger buffer capacity to use for the next operating system call attempt.
    ///
    /// # Arguments
    ///
    /// - `tries` - The number of times the operating system call has been attempted.  Count starts
    /// at 1.
    /// - `desired_capacity` - There are two possible meanings for this argument...
    ///     - If the operating system returns the number of elements stored, like
    ///         [`GetUserNameW`][1], then this argument is the number of bytes stored.  The
    ///         expectation is that `next_capacity` uses this value as the floor and chooses the
    ///         next capacity to be reasonably larger.
    ///     - If the operating system returns the capacity needed for success, like
    ///         [`GetLogicalProcessorInformationEx`][1], then this argument is the capacity, in
    ///         bytes, needed.  The expectation is that `next_capacity` returns something no less
    ///         than and not too much greater than this value.
    ///
    /// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/WindowsProgramming/fn.GetUserNameW.html
    /// [2]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/SystemInformation/fn.GetLogicalProcessorInformationEx.html
    ///
    fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32;
}

/// Used internally help determine the [`FillBufferAction`][1].
///
/// Specifically, [`to_result`][tr] is passed a `NeededSize`.
///
/// [1]: crate::base::FillBufferAction
/// [tr]: crate::ToResult::to_result
///
pub trait NeededSize {
    fn needed_size(&self) -> u32;
    fn set_needed_size(&mut self, value: u32);
}

/// Conversion between capacity (bytes in the buffer) and size (API units of measure like WCHARs).
/// Conversion to the API pointer type.
///
pub trait RawToInternal {
    /// Converts from a buffer capacity, in bytes, to an operating system size, like number of WCHARs.
    ///
    /// For operating system calls that return binary data, size and capacity are both in bytes.
    ///
    fn capacity_to_size(value: u32) -> u32;
    /// Converts from a generic `*mut u8` pointer to a typed pointer.
    ///
    /// Some operating system calls prefer a typed pointer.  This method casts the low-level buffer
    /// pointer to a correctly typed pointer.
    ///
    fn convert_pointer(value: *mut u8) -> Self;
    /// Converts from an operating system size, like number of WCHARs, to a buffer capacity, in bytes.
    ///
    /// For operating system calls that return binary data, size and capacity are both in bytes.
    ///
    fn size_to_capacity(value: u32) -> u32;
}

impl<T> RawToInternal for *mut T {
    fn capacity_to_size(value: u32) -> u32 {
        value
    }
    fn convert_pointer(value: *mut u8) -> *mut T {
        value as *mut T
    }
    fn size_to_capacity(value: u32) -> u32 {
        value
    }
}

/// Return a read-only pointer to a buffer and the actual number of bytes stored in the buffer.
///
/// This trait is used internally by [`read_buffer`][rb] to provide read-only access to a buffer
/// after the operating system call was successful.
///
/// [rb]: crate::FrozenBuffer::read_buffer
///
pub trait ReadBuffer {
    /// Returns a pointer to the data and the number of elements (`FT`s) stored.
    fn read_buffer(&self) -> (Option<*const u8>, u32);
}

/// Management for a writable buffer.
///
pub trait WriteBuffer {
    /// Returns a [`ReadBuffer`] view of a buffer.
    ///
    /// `as_read_buffer` is used to help prepare a [`FrozenBuffer`][fb] after the operating system
    /// call succeeds.
    ///
    /// [fb]: crate::FrozenBuffer
    ///
    fn as_read_buffer(&self) -> &dyn ReadBuffer;
    /// Returns the capacity, in bytes, of this buffer.
    ///
    /// `capacity` is used internally for safety checks.  `write_buffer` provides a raw pointer and
    /// size for writing into the buffer.
    ///
    fn capacity(&self) -> u32;
    /// Called from [`freeze`][f] to establish the amount of data written to the buffer.
    ///
    /// As part of creating the returned [`FrozenBuffer`][fb], [`freeze`][f] calls this method to
    /// establish the amount of data available through the [`FrozenBuffer`][fb].
    ///
    /// [f]: crate::GrowableBuffer::freeze
    /// [fb]: crate::FrozenBuffer
    ///
    fn set_final_size(&mut self, final_size: u32);
    /// Return a raw correctly aligned pointer into the buffer and the capacity of the buffer in bytes.
    ///
    /// `write_buffer` is used by [`GrowableBuffer`][gb], in the [`argument`][am] method, to prepare
    /// an [`Argument`][a].
    ///
    /// [gb]: crate::GrowableBuffer
    /// [am]: crate::GrowableBuffer::argument
    /// [a]: crate::Argument
    ///
    fn write_buffer(&mut self) -> (*mut u8, u32);
}

/// Convert an API return value and the needed buffer size into a `FillBufferResult` which is then
/// converted to a [`FillBufferAction`][1].
///
/// [1]: crate::base::FillBufferAction
pub trait ToResult {
    /// Details for `to_result` are available with the [`RvIsError::to_result`][e] and
    /// [`RvIsSize::to_result`][s] implementations.
    ///
    /// [e]: crate::RvIsError::to_result
    /// [s]: crate::RvIsSize::to_result
    fn to_result(&self, needed_size: &mut dyn NeededSize) -> FillBufferResult;
}
