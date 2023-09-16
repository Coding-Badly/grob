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

pub trait GrowStrategy {
    fn next_capacity(&self, tries: usize, current_size: u32, desired_size: u32) -> u32;
}

pub trait NeededSize {
    fn needed_size(&self) -> u32;
    fn set_needed_size(&mut self, value: u32);
}

pub trait RawToInternal {
    fn capacity_to_size(value: u32) -> u32;
    fn convert_pointer(value: *mut u8) -> Self;
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

pub trait ReadBuffer {
    fn read_buffer(&self) -> (Option<*const u8>, u32);
}

pub trait WriteBuffer {
    fn as_read_buffer(&self) -> &dyn ReadBuffer;
    fn capacity(&self) -> u32;
    fn set_final_size(&mut self, final_size: u32);
    fn write_buffer(&mut self) -> (*mut u8, u32);
}

pub trait ToResult {
    fn to_result(&self, needed_size: &mut dyn NeededSize) -> FillBufferResult;
}
