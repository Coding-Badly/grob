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

//! Welcome to the [grob (growable buffer) crate][gc]!
//!
//! [gc]: https://crates.io/crates/grob
//!
//! Many Windows API functions require the caller to provide a buffer for the returned data.  The
//! pattern goes something like this...
//!
//! * Call the function with an initial buffer and size
//! * If that works then process the returned data
//! * If that does not work because the buffer is too small then create a larger buffer and try again
//! * If that does not work for any other reason then deal with the error
//!
//! There are copious examples of a growable buffer including a version in the Rust Standard
//! Library.  There is a lack of consistency amoungst all the examples.  Some versions continue
//! trying indefinately.  Some versions make an arbitrary number of attempts like three then give
//! up.  Some versions double the size of the new buffer.  Some versions increase the size by a
//! fixed amount like 128 bytes.  Even Microsoft's API examples are inconsistent.
//!
//! The goal with this crate is to provide a single high quality growable buffer that any Rust
//! developer can easily use.
//!
//! # Getting Start
//!
//! The [generic functions](#functions) listed below are a great place to start.  They wrap all the
//! details necessary for making Windows API calls.
//!
//! | When the API Call Returns | And the Data Is | For Example                             | Use                                     |
//! | ------------------------- | --------------- | --------------------------------------- | --------------------------------------- |
//! | an error code; a [`u32`]  | large + binary  | [`GetTcpTable2`][4]                     | [`winapi_large_binary`] + [`RvIsError`] |
//! | an error code; a [`u32`]  | large + binary  | [`GetAdaptersAddresses`][1]             | [`winapi_large_binary`] + [`RvIsError`] |
//! | a [`BOOL`][b]             | small + binary  | [`GetLogicalProcessorInformationEx`][2] | [`winapi_small_binary`] + [`RvIsError`] |
//! | a [`BOOL`][b]             | text            | [`GetUserNameW`][5]                     | [`winapi_string`] + [`RvIsError`]       |
//! | elements / WCHARs stored  | path            | [`GetModuleFileNameW`][3]               | [`winapi_path_buf`] + [`RvIsSize`]      |
//! | elements / WCHARs stored  | path            | [`GetSystemWindowsDirectoryW`][6]       | [`winapi_path_buf`] + [`RvIsSize`]      |
//! | bytes stored              | large + binary  | [`GetFileVersionInfoSizeW`][7]          | [`winapi_large_binary`] + [see example][e] |
//!
//! [b]: windows::Win32::Foundation::BOOL
//! [1]: https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses
//! [2]: https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getlogicalprocessorinformationex
//! [3]: https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulefilenamew
//! [4]: https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-gettcptable2
//! [5]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getusernamew
//! [6]: https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemwindowsdirectoryw
//! [7]: https://learn.microsoft.com/en-us/windows/win32/api/winver/nf-winver-getfileversioninfosizew
//! [e]: https://github.com/Coding-Badly/grob/blob/main/grob/examples/version-info-generic.rs
//!

use std::marker::PhantomData;

mod base;
mod buffer;
mod generic;
mod strategy;
mod traits;
mod win;

pub use crate::base::{FillBufferAction, FillBufferResult};
pub use crate::buffer::{os::ALIGNMENT, StackBuffer};
pub use crate::generic::{
    winapi_binary, winapi_generic, winapi_large_binary, winapi_path_buf, winapi_small_binary,
    winapi_string,
};
pub use crate::strategy::{
    GrowByDoubleWithNull, GrowForSmallBinary, GrowForStaticText, GrowForStoredIsReturned,
    GrowToNearestNibble, GrowToNearestNibbleWithNull, GrowToNearestQuarterKibi,
};
pub use crate::traits::{
    GrowStrategy, NeededSize, RawToInternal, ReadBuffer, ToResult, WriteBuffer,
};
pub use crate::win::{RvIsError, RvIsSize, CAPACITY_FOR_NAMES, CAPACITY_FOR_PATHS, SIZE_OF_WCHAR};

use crate::buffer::HeapBuffer;
use crate::traits::GrowableBufferAsParent;

enum ActiveBuffer<'sb> {
    Heap(HeapBuffer),
    Initial(&'sb mut dyn WriteBuffer),
    PendingSwitch,
}

impl<'sb> ActiveBuffer<'sb> {
    pub fn set_final_size(&mut self, final_size: u32) {
        match self {
            Self::Heap(h) => h.set_final_size(final_size),
            Self::Initial(wb) => wb.set_final_size(final_size),
            Self::PendingSwitch => panic!("PendingSwitch is only valid in grow"),
        }
    }
}

struct BufferStrategy<'gs, 'sb> {
    active_buffer: ActiveBuffer<'sb>,
    grow_strategy: &'gs dyn GrowStrategy,
    tries: usize,
}

impl<'gs, 'sb> BufferStrategy<'gs, 'sb> {
    fn capacity(&self) -> u32 {
        match &self.active_buffer {
            ActiveBuffer::Heap(h) => h.capacity(),
            ActiveBuffer::Initial(wb) => wb.capacity(),
            ActiveBuffer::PendingSwitch => panic!("PendingSwitch is only valid in grow"),
        }
    }
    fn grow(&mut self, desired_capacity: u32) {
        let current_capacity = self.capacity();
        // nfx? Do we need this check? A bug elsewhere could cause an infinite loop. `grow` should
        // only be called when we know for certain the buffer needs to grow.
        // nfx? Should it be an assertion?
        if desired_capacity > current_capacity {
            self.tries += 1;
            let adjusted_capacity = self
                .grow_strategy
                .next_capacity(self.tries, desired_capacity);
            // We were told to grow the buffer.  If that did not happen we have a bug.
            assert!(adjusted_capacity > current_capacity);
            // If we're holding a heap allocated buffer then free it now.  This allows the heap
            // manager to reuse the memory we just released for our larger allocation.
            self.active_buffer = ActiveBuffer::PendingSwitch;
            self.active_buffer = ActiveBuffer::Heap(HeapBuffer::new(adjusted_capacity));
        }
    }
    fn raw_buffer(&mut self) -> (*mut u8, u32) {
        match &mut self.active_buffer {
            ActiveBuffer::Heap(h) => h.write_buffer(),
            ActiveBuffer::Initial(wb) => wb.write_buffer(),
            ActiveBuffer::PendingSwitch => panic!("PendingSwitch is only valid in grow"),
        }
    }
}

struct EmptyReadBuffer {}

impl ReadBuffer for EmptyReadBuffer {
    fn read_buffer(&self) -> (Option<*const u8>, u32) {
        (None, 0)
    }
}
const EMPTY_READ_BUFFER: EmptyReadBuffer = EmptyReadBuffer {};

enum PassiveBuffer<'sb> {
    Heap(HeapBuffer),
    Initial(&'sb dyn ReadBuffer),
}

impl<'sb> From<ActiveBuffer<'sb>> for PassiveBuffer<'sb> {
    fn from(value: ActiveBuffer<'sb>) -> Self {
        match value {
            ActiveBuffer::Heap(h) => PassiveBuffer::Heap(h),
            ActiveBuffer::Initial(s) => PassiveBuffer::Initial(s.as_read_buffer()),
            ActiveBuffer::PendingSwitch => panic!("PendingSwitch is only valid in grow"),
        }
    }
}

/// Read-only buffer filled with data from an operating system call.
///
/// [`GrowableBuffer::freeze`] returns a [`FrozenBuffer`].  If the operating system call was
/// successful then the [`FrozenBuffer`] contains the data.  If the call was not successful then an
/// empty [`FrozenBuffer`] is returned.
///
pub struct FrozenBuffer<'sb, FT> {
    passive_buffer: PassiveBuffer<'sb>,
    final_type: PhantomData<FT>,
}

impl<'sb, FT> FrozenBuffer<'sb, FT> {
    /// Returns a pointer to the data and the number of elements (`FT`s) stored.
    ///
    /// Do not read past the end of the buffer.  If zero elements were stored do not dereference
    /// the pointer.  Doing either is undefined behaviour.
    ///
    /// If the initial buffer was frozen and was too small to meet the alignment requirement then
    /// [`None`] is returned instead of a pointer.
    ///
    // nfx? Return null if the number elements stored is zero? Return None instead?
    pub fn read_buffer(&self) -> (Option<*const FT>, u32) {
        let (p, s) = match &self.passive_buffer {
            PassiveBuffer::Heap(h) => h.read_buffer(),
            PassiveBuffer::Initial(wb) => wb.read_buffer(),
        };
        (p.map(|p| p as *const FT), s)
    }
    /// Returns a pointer to the data.
    ///
    /// If the initial buffer was frozen and is too small to meet the alignment requirement then
    /// [`None`] is returned instead of a pointer.
    ///
    pub fn pointer(&self) -> Option<*const FT> {
        self.read_buffer().0
    }
    /// Returns the number of elements (`FT`s) stored.
    ///
    /// Do not read past the end of the buffer.  If zero elements were stored do not dereference
    /// the pointer.  Doing either is undefined behaviour.
    ///
    pub fn size(&self) -> u32 {
        self.read_buffer().1
    }
}

/// Wrapper for Windows API arguments.  Typically a pointer to the buffer and a pointer to the
/// buffer size or a `&mut [T]`.
///
/// `Argument` is the bridge between an operating system call and a buffer.  Typical activities
/// include providing arguments for an operating system call, growing the buffer, and finalizing
/// the buffer.
///
pub struct Argument<'gb, IT> {
    parent: &'gb mut dyn GrowableBufferAsParent,
    pointer: IT,
    size: u32,
    tries: usize,
}

impl<'gb, IT> Argument<'gb, IT>
where
    IT: Copy,
{
    /// Apply an action ([`FillBufferAction`]) to the underlying buffer.
    ///
    /// `apply` is called in response to the return value from an operating system call.  If the
    /// operating system indicates success then `apply` is called with [`FillBufferAction::Commit`]
    /// if data is available or [`FillBufferAction::NoData`] if no data is available.  If the
    /// operating system indicates the buffer is too small then `apply` is called with
    /// [`FillBufferAction::Grow`] to apply the [`GrowStrategy`] to the buffer.
    ///
    /// [`Argument`] is no longer valid after the call to `apply` so `apply` consumes the
    /// [`Argument`].
    ///
    /// `apply` is only needed when using the low-level code.  The generic functions
    /// ([`winapi_large_binary`], [`winapi_path_buf`], [`winapi_small_binary`], and
    /// [`winapi_string`]) call `apply` automatically.
    ///
    pub fn apply(self, fill_buffer_action: FillBufferAction) -> bool {
        match fill_buffer_action {
            FillBufferAction::Commit => {
                self.commit();
                true
            }
            FillBufferAction::Grow => {
                self.grow();
                false
            }
            FillBufferAction::NoData => {
                self.commit_no_data();
                true
            }
        }
    }
    /// Set the final size of the buffer so the data is ready to be used.
    ///
    /// Calling this method is rarely necessary.  Normally it's called from [`apply`][1].  Calling
    /// `commit` directly will be necessary if a return value handler ([`RvIsError`] or
    /// [`RvIsSize`]) is not adequate for converting an operating system return value into a
    /// [`FillBufferAction`].
    ///
    /// [1]: [Argument::apply]
    ///
    pub fn commit(self) {
        self.parent.set_final_size(self.size);
    }
    /// Set the final size of the buffer to zero indicating the operating system call was successful
    /// but did not return any data.
    ///
    /// Calling this method is rarely necessary.  Normally it's called from [`apply`][1].  Calling
    /// `commit_no_data` directly will be necessary if a return value handler ([`RvIsError`] or
    /// [`RvIsSize`]) is not adequate for converting an operating system return value into a
    /// [`FillBufferAction`].
    ///
    /// [1]: [Argument::apply]
    ///
    pub fn commit_no_data(self) {
        self.parent.set_final_size(0);
    }
    /// Increase the amount of space available in the buffer using the [`GrowStrategy`].
    ///
    /// Calling this method is rarely necessary.  Normally it's called from [`apply`][1].  Calling
    /// `grow` directly will be necessary if a return value handler ([`RvIsError`] or [`RvIsSize`])
    /// is not adequate for converting an operating system return value into a [`FillBufferAction`].
    ///
    /// [1]: [Argument::apply]
    ///
    pub fn grow(self) {
        self.parent.grow(self.size);
    }
    /// Returns a correctly typed pointer to the buffer, ready to be used for an operating system
    /// call.
    ///
    /// For example, The `lpbuffer` parameter for [`GetUserNameW`][1] is a `PWSTR`.  When [grob] is
    /// used correctly, `pointer` returns a `PWSTR`.
    ///
    /// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/WindowsProgramming/fn.GetUserNameW.html
    /// [grob]: https://crates.io/crates/grob
    ///
    pub fn pointer(&self) -> IT {
        self.pointer
    }
    /// Returns a correctly typed pointer to the buffer size, ready to be used for an operating
    /// system call.
    ///
    /// For example, The `pcbbuffer` parameter for [`GetUserNameW`][1] is a `*mut u32`.  The `size`
    /// method returns a `*mut u32`.  The referenced value is initialized to the current size of the
    /// buffer.
    ///
    /// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/WindowsProgramming/fn.GetUserNameW.html
    /// [grob]: https://crates.io/crates/grob
    ///
    pub fn size(&mut self) -> *mut u32 {
        &mut self.size
    }
    /// Returns the number of attempts that have been made.
    ///
    /// `tries` is only used by the Miri tests.  It is unstable (e.g. may be removed or changed in
    /// any future version).
    ///
    pub fn tries(&self) -> usize {
        self.tries
    }
}

impl<'gb, IT> NeededSize for Argument<'gb, IT> {
    /// Return the buffer size needed by the operating system to fulfill the request.
    ///
    /// Before the operating system call, `needed_size` returns the size of the buffer.
    ///
    /// After the call, for calls that return the buffer size needed through a pointer to a [`u32`],
    /// `needed_size` returns that value.
    ///
    /// After the call, for calls that return the number stored as the return value of the function,
    /// `needed_size` continues to return the size of the buffer.
    ///
    /// `needed_size` is used internally by [`RvIsError`] and [`RvIsSize`] to grow the buffer as
    /// needed and terminate the call loop on success.
    ///
    fn needed_size(&self) -> u32 {
        self.size
    }
    /// Called to indicate how many bytes were stored or to set the next buffer size to try.
    ///
    /// If the current buffer was big enough `set_needed_size` is called with the number of elements
    /// stored.
    ///
    /// If the current buffer was not big enough then `set_needed_size` is called with the number of
    /// elements stored, which is expected to also be the current buffer size, multiplied by two.
    /// The net effect should be the buffer doubles in size with each attempt.
    ///
    /// `set_needed_size` is used internally by [`RvIsSize`] for operating system calls that return
    /// the number of elements (characters) stored.
    ///
    fn set_needed_size(&mut self, value: u32) {
        self.size = value;
    }
}

/// Writable buffer capable of providing an [`Argument`] for a Windows API function then a
/// [`FrozenBuffer`] when that call succeeds.
///
/// `GrowableBuffer` is the core component of the [grob (growable buffer) crate][gc].  It brings
/// together an initial [`StackBuffer`] and a [`GrowStrategy`] to help iteratively call a Windows
/// API function until that call succeeds with a reasonably sized buffer.
///
/// [gc]: https://crates.io/crates/grob
pub struct GrowableBuffer<'gs, 'sb, FT, IT> {
    final_size: u32,
    buffer_strategy: BufferStrategy<'gs, 'sb>,
    final_type: PhantomData<FT>,
    intermediate_type: PhantomData<IT>,
}

impl<'gs, 'sb, FT, IT> GrowableBuffer<'gs, 'sb, FT, IT>
where
    IT: RawToInternal,
{
    /// Create a [`GrowableBuffer`] from an initial [`StackBuffer`] and a [`GrowStrategy`].
    ///
    /// # Arguments
    ///
    /// * `initial` - The initial buffer.  Typically this is a reasonably sized [`StackBuffer`].
    /// A zero sized [`StackBuffer`] can be passed to force the use of a heap buffer.  Using a heap
    /// buffer allows moving the data more efficiently; the buffer can be easily "carried away".
    /// * `grow_strategy` - Determines how the heap buffer should grow.  This crate provides two
    /// basic strategies: double the size ([`GrowByDoubleWithNull`]) or use the size requested
    /// ([`GrowToNearestNibble`], [`GrowToNearestNibbleWithNull`], [`GrowToNearestQuarterKibi`]).
    ///
    pub fn new(initial: &'sb mut dyn WriteBuffer, grow_strategy: &'gs dyn GrowStrategy) -> Self {
        let buffer_strategy = BufferStrategy {
            active_buffer: ActiveBuffer::Initial(initial),
            grow_strategy,
            tries: 0,
        };
        Self {
            final_size: 0,
            buffer_strategy,
            final_type: PhantomData,
            intermediate_type: PhantomData,
        }
    }
    /// Convert a [`GrowableBuffer`] to a [`FrozenBuffer`].
    ///
    /// `freeze` is called after the Windows API function returns success.  While it can be called
    /// at any time, if the API function was not successful, the returned [`FrozenBuffer`] will be
    /// empty (have a size of zero).
    ///
    /// The data stored by the API function is accessible through the returned [`FrozenBuffer`].
    ///
    /// # Arguments
    ///
    /// * `self` - The [`GrowableBuffer`] used when calling the Windows API function.
    ///
    pub fn freeze(self) -> FrozenBuffer<'sb, FT> {
        let GrowableBuffer {
            final_size,
            buffer_strategy,
            ..
        } = self;
        let passive_buffer = if final_size > 0 {
            let mut active_buffer = buffer_strategy.active_buffer;
            active_buffer.set_final_size(final_size);
            active_buffer.into()
        } else {
            PassiveBuffer::Initial(&EMPTY_READ_BUFFER)
        };
        FrozenBuffer {
            passive_buffer,
            final_type: PhantomData,
        }
    }
    /// Return an [`Argument`] that provides the argument(s) for calling a Windows API function
    ///
    /// `argument` is called before the Windows API function to get an [`Argument`] instance for the
    /// [`GrowableBuffer`].  [`Argument`] has methods for getting the low-level argument(s) that are
    /// passed to the Windows API function.  It also has methods for finalizing the buffer size on
    /// success and growing the buffer if it's too small.
    ///
    /// # Notes
    ///
    /// * The returned [`Argument`] carries a mutable reference to the [`GrowableBuffer`].  This
    /// ensures the buffer cannot be changed or dropped until after the operating system call (while
    /// the [`Argument`] instance exists).  It also ensures there can be only zero or one
    /// [`Argument`] at any moment.
    ///
    pub fn argument(&mut self) -> Argument<'_, IT> {
        self.final_size = 0;
        let (pointer, capacity) = self.buffer_strategy.raw_buffer();
        let tries = self.buffer_strategy.tries + 1;
        Argument {
            parent: self as &mut dyn GrowableBufferAsParent,
            pointer: IT::convert_pointer(pointer),
            size: IT::capacity_to_size(capacity),
            tries,
        }
    }
}

impl<'gs, 'sb, FT, IT> GrowableBufferAsParent for GrowableBuffer<'gs, 'sb, FT, IT>
where
    IT: RawToInternal,
{
    fn grow(&mut self, size: u32) {
        self.buffer_strategy.grow(IT::size_to_capacity(size));
    }
    fn set_final_size(&mut self, size: u32) {
        let needed_capacity = IT::size_to_capacity(size);
        assert!(needed_capacity <= self.buffer_strategy.capacity());
        self.final_size = size;
    }
}
