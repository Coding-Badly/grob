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

use std::marker::PhantomData;

mod base;
mod buffer;
mod generic;
mod strategy;
mod traits;
mod win;

pub use crate::base::FillBufferAction;
pub use crate::buffer::{os::ALIGNMENT, StackBuffer};
pub use crate::generic::{
    winapi_binary, winapi_generic, winapi_large_binary, winapi_path_buf, winapi_small_binary,
    winapi_string,
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
        if desired_capacity > current_capacity {
            self.tries += 1;
            let adjusted_capacity =
                self.grow_strategy
                    .next_capacity(self.tries, current_capacity, desired_capacity);
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

pub struct FrozenBuffer<'sb, FT> {
    passive_buffer: PassiveBuffer<'sb>,
    final_type: PhantomData<FT>,
}

impl<'sb, FT> FrozenBuffer<'sb, FT> {
    // nfx? Return null if the buffer size is zero?
    pub fn read_buffer(&self) -> (Option<*const FT>, u32) {
        let (p, s) = match &self.passive_buffer {
            PassiveBuffer::Heap(h) => h.read_buffer(),
            PassiveBuffer::Initial(wb) => wb.read_buffer(),
        };
        (p.map(|p| p as *const FT), s)
    }
    pub fn pointer(&self) -> Option<*const FT> {
        self.read_buffer().0
    }
    pub fn size(&self) -> u32 {
        self.read_buffer().1
    }
}

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
    pub fn commit(self) {
        self.parent.set_final_size(self.size);
    }
    pub fn commit_no_data(self) {
        self.parent.set_final_size(0);
    }
    pub fn grow(self) {
        self.parent.grow(self.size);
    }
    pub fn pointer(&self) -> IT {
        self.pointer
    }
    pub fn size(&mut self) -> *mut u32 {
        &mut self.size
    }
    pub fn tries(&self) -> usize {
        self.tries
    }
}

impl<'gb, IT> NeededSize for Argument<'gb, IT> {
    fn needed_size(&self) -> u32 {
        self.size
    }
    fn set_needed_size(&mut self, value: u32) {
        self.size = value;
    }
}

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
        self.final_size = size;
    }
}
