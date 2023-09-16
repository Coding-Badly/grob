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
    pub const ALIGNMENT: usize = MEMORY_ALLOCATION_ALIGNMENT as usize;
}

#[cfg(not(windows))]
pub(crate) mod os {
    pub const ALIGNMENT: usize = 8;
}

use crate::traits::{ReadBuffer, WriteBuffer};

pub struct StackBuffer<const CAPACITY: usize> {
    final_size: u32,
    stack: MaybeUninit<[u8; CAPACITY]>,
}

impl<const CAPACITY: usize> StackBuffer<CAPACITY> {
    pub fn new() -> Self {
        Self {
            final_size: 0,
            stack: MaybeUninit::uninit(),
        }
    }
    fn as_mut_ptr(&mut self) -> (*mut u8, usize) {
        // let offset = p.addr() % ALIGNMENT;
        // https://github.com/rust-lang/rust/issues/95228
        let p = self.stack.as_mut_ptr() as *mut u8;
        let offset = (p as usize) % os::ALIGNMENT;
        (unsafe { p.add(offset) }, offset)
    }
    fn as_ptr(&self) -> (*const u8, usize) {
        // let offset = p.addr() % ALIGNMENT;
        // https://github.com/rust-lang/rust/issues/95228
        let p = self.stack.as_ptr() as *const u8;
        let offset = (p as usize) % os::ALIGNMENT;
        (unsafe { p.add(offset) }, offset)
    }
    fn capacity(&self) -> u32 {
        if CAPACITY >= os::ALIGNMENT {
            (CAPACITY - self.offset()).try_into().unwrap()
        } else {
            0
        }
    }
    fn offset(&self) -> usize {
        let p = self.stack.as_ptr() as *const u8;
        (p as usize) % os::ALIGNMENT
    }
}

impl<const CAPACITY: usize> Default for StackBuffer<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAPACITY: usize> ReadBuffer for StackBuffer<CAPACITY> {
    fn read_buffer(&self) -> (Option<*const u8>, u32) {
        if CAPACITY >= os::ALIGNMENT {
            (Some(self.as_ptr().0), self.final_size)
        } else {
            (None, 0)
        }
    }
}

impl<const CAPACITY: usize> WriteBuffer for StackBuffer<CAPACITY> {
    fn as_read_buffer(&self) -> &dyn ReadBuffer {
        self as &dyn ReadBuffer
    }
    fn capacity(&self) -> u32 {
        if CAPACITY >= os::ALIGNMENT {
            (CAPACITY - self.offset()).try_into().unwrap()
        } else {
            0
        }
    }
    fn set_final_size(&mut self, final_size: u32) {
        assert!(final_size <= self.capacity());
        self.final_size = final_size;
    }
    fn write_buffer(&mut self) -> (*mut u8, u32) {
        if CAPACITY >= os::ALIGNMENT {
            let (p, o) = self.as_mut_ptr();
            (p, (CAPACITY - o).try_into().unwrap())
        } else {
            (std::ptr::null_mut(), 0)
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
    fn capacity(&self) -> u32 {
        self.capacity
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
    pub(crate) fn set_final_size(&mut self, final_size: u32) {
        assert!(final_size <= self.capacity());
        self.final_size = final_size;
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
        assert!(final_size <= self.capacity());
        self.final_size = final_size;
    }
    fn write_buffer(&mut self) -> (*mut u8, u32) {
        (self.pointer, self.capacity)
    }
}
