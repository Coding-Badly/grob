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

use std::alloc::{alloc, dealloc, Layout};
use std::mem::MaybeUninit;

#[cfg(windows)]
pub(crate) mod os {
    use windows::Win32::System::SystemServices::MEMORY_ALLOCATION_ALIGNMENT;

    /// Buffer alignment that works for all Windows API calls; alignment used for all grob buffers
    ///
    /// This value is unlikely to be useful outside of the [grob crate][gc].  The value is taken
    /// from the windows crate ([`MEMORY_ALLOCATION_ALIGNMENT`]) and cast as [`usize`] to make it
    /// more Rust friendly.
    ///
    /// [gc]: https://crates.io/crates/grob
    ///
    pub const ALIGNMENT: usize = MEMORY_ALLOCATION_ALIGNMENT as usize;
}

#[cfg(not(windows))]
pub(crate) mod os {
    /// Buffer alignment that works for all operating system calls (experimental)
    pub const ALIGNMENT: usize = 8;
}

use crate::traits::{ReadBuffer, WriteBuffer};

/// Initial buffer placed on the stack to improve performance.
///
/// The [grob crate][gc] supports an initial [`StackBuffer`] to improve performance.  If the
/// [`StackBuffer`] is too small then [`GrowableBuffer`][gb] switches to a heap buffer.
///
/// A [`StackBuffer`] can be zero-sized.  When the [`StackBuffer`] is zero-sized,
/// [`GrowableBuffer`][gb] makes an operating system call to determine a best guess for the initial
/// heap buffer size.
///
/// Ideally, a [`StackBuffer`] is sized so switching to a heap buffer is rarely necessary.  The
/// [grob crate][gc] provides two constants to help avoid switching to a heap buffer:
/// [`CAPACITY_FOR_NAMES`][cfn] and [`CAPACITY_FOR_PATHS`][cfp]
///
/// # Examples
///
/// ``` ignore
///     let mut initial_buffer = StackBuffer::<CAPACITY_FOR_PATHS>::new();
///     let grow_strategy = GrowForStoredIsReturned::<CAPACITY_FOR_PATHS>::new();
///     let mut growable_buffer = GrowableBuffer::<u16, PWSTR>::new(initial_buffer, &grow_strategy);
///     loop {
///         let mut argument = growable_buffer.argument();
///         let rv = unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) };
///         let rv: RvIsSize = rv.into();
///         let result = rv.to_result(&mut argument);
///         // react to result
///     }
/// ```
///
/// [gc]: https://crates.io/crates/grob
/// [gb]: crate::GrowableBuffer
/// [cfn]: crate::CAPACITY_FOR_NAMES
/// [cfp]: crate::CAPACITY_FOR_PATHS
///
pub struct StackBuffer<const CAPACITY: usize> {
    final_size: u32,
    stack: MaybeUninit<[u8; CAPACITY]>,
}

impl<const CAPACITY: usize> StackBuffer<CAPACITY> {
    /// Constructs a stack buffer of size `CAPACITY`.
    pub fn new() -> Self {
        Self {
            final_size: 0,
            stack: MaybeUninit::uninit(),
        }
    }
    fn as_mut_ptr(&mut self) -> (*mut u8, usize) {
        // nfx: Future enhancement...
        // https://github.com/rust-lang/rust/issues/95228
        let p = self.stack.as_mut_ptr() as *mut u8;
        let offset = p.align_offset(os::ALIGNMENT);
        (unsafe { p.add(offset) }, offset)
    }
    fn as_ptr(&self) -> (*const u8, usize) {
        // nfx: Future enhancement...
        // https://github.com/rust-lang/rust/issues/95228
        let p = self.stack.as_ptr() as *const u8;
        let offset = p.align_offset(os::ALIGNMENT);
        (unsafe { p.add(offset) }, offset)
    }
    fn offset(&self) -> usize {
        let p = self.stack.as_ptr() as *const u8;
        p.align_offset(os::ALIGNMENT)
    }
}

impl<const CAPACITY: usize> Default for StackBuffer<CAPACITY> {
    /// Constructs a stack buffer of size `CAPACITY`.
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAPACITY: usize> ReadBuffer for StackBuffer<CAPACITY> {
    /// Returns a read-only pointer to the buffer and the number of elements stored in the buffer.
    ///
    /// If the buffer is too small to meet the alignment needed by the operating system then
    /// `(none, 0)` is returned.
    ///
    /// `read_buffer` is used by [`FrozenBuffer`][fb] to provide access to the data stored by the
    /// operating system.
    ///
    /// [fb]: crate::FrozenBuffer
    ///
    fn read_buffer(&self) -> (Option<*const u8>, u32) {
        if CAPACITY >= os::ALIGNMENT {
            (Some(self.as_ptr().0), self.final_size)
        } else {
            (None, 0)
        }
    }
}

impl<const CAPACITY: usize> WriteBuffer for StackBuffer<CAPACITY> {
    /// Returns the [`ReadBuffer`] for this [`StackBuffer`].
    ///
    /// `as_read_buffer` is used internally when converting to a [`FrozenBuffer`][fb].
    ///
    /// [fb]: crate::FrozenBuffer
    ///
    fn as_read_buffer(&self) -> &dyn ReadBuffer {
        self as &dyn ReadBuffer
    }
    /// Returns the available capacity for this [`StackBuffer`].
    ///
    /// The operating system expects buffers to be aligned on [`ALIGNMENT`][a] boundaries.  Rust
    /// guarentees alignment to the size of each array element.  Internally [`StackBuffer`] uses an
    /// array of [`u8`] so the buffer is aligned to the nearest byte (not aligned).  `capacity` may
    /// be reduced so a correctly aligned buffer can be presented to the operating system.  In other
    /// words, a 256 byte buffer may be reduced to a capacity of 241 bytes
    /// (256 - ([`ALIGNMENT`][a] - 1)).
    ///
    /// [a]: os::ALIGNMENT
    ///
    fn capacity(&self) -> u32 {
        if CAPACITY >= os::ALIGNMENT {
            (CAPACITY - self.offset()).try_into().unwrap()
        } else {
            0
        }
    }
    /// Called from [`freeze`][f] to set the amount of data provided by the operating system.
    ///
    /// When the buffer used by [`GrowableBuffer`][gb] is turned into a [`FrozenBuffer`][fb] the
    /// amount of data stored by the operating system is included.  This allows the stored data to
    /// be safely accessed.
    ///
    /// [f]: crate::GrowableBuffer::freeze
    /// [gb]: crate::GrowableBuffer
    /// [fb]: crate::FrozenBuffer
    ///
    fn set_final_size(&mut self, final_size: u32) {
        self.final_size = final_size;
    }
    /// Returns a pointer and size allowing write access to the buffer.
    ///
    /// This method is used indirectly by [`Argument`][a] to provide suitable arguments for an
    /// operating system call.
    ///
    /// [a]: crate::Argument
    ///
    fn write_buffer(&mut self) -> (*mut u8, u32) {
        if CAPACITY >= os::ALIGNMENT {
            let (p, o) = self.as_mut_ptr();
            (p, (CAPACITY - o).try_into().unwrap())
        } else {
            // This pointer may not be aligned but we're indicating there's zero capacity available
            // so the caller had better not be using it.
            let p = self.stack.as_mut_ptr() as *mut u8;
            (p, 0)
        }
    }
}

pub(crate) struct HeapBuffer {
    capacity: u32,
    final_size: u32,
    layout: Layout,
    pointer: *mut u8,
}

impl HeapBuffer {
    pub(crate) fn new(capacity: u32) -> Self {
        let layout = Layout::from_size_align(capacity.try_into().unwrap(), os::ALIGNMENT).unwrap();
        let pointer = unsafe { alloc(layout) };
        if pointer.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Self {
            capacity,
            final_size: 0,
            layout,
            pointer,
        }
    }
}

impl Drop for HeapBuffer {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            unsafe { dealloc(self.pointer, self.layout) };
        }
    }
}

impl HeapBuffer {
    pub(crate) fn read_buffer(&self) -> (Option<*const u8>, u32) {
        assert!(self.final_size > 0);
        (Some(self.pointer), self.final_size)
    }
}

impl ReadBuffer for HeapBuffer {
    fn read_buffer(&self) -> (Option<*const u8>, u32) {
        assert!(self.final_size > 0);
        (Some(self.pointer), self.final_size)
    }
}

impl WriteBuffer for HeapBuffer {
    fn as_read_buffer(&self) -> &dyn ReadBuffer {
        self as &dyn ReadBuffer
    }
    fn capacity(&self) -> u32 {
        self.capacity
    }
    fn set_final_size(&mut self, final_size: u32) {
        self.final_size = final_size;
    }
    fn write_buffer(&mut self) -> (*mut u8, u32) {
        (self.pointer, self.capacity)
    }
}
