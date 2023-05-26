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

use crate::buffer::os::ALIGNMENT;
use crate::traits::GrowStrategy;
use crate::win::SIZE_OF_WCHAR;

pub struct GrowByNearestNibble {}

impl GrowByNearestNibble {
    pub fn new() -> Self {
        Self {}
    }
}

impl GrowStrategy for GrowByNearestNibble {
    fn next_capacity(&self, _tries: usize, current_size: u32, desired_size: u32) -> u32 {
        // With desired_size a u32, doing the math with u64 prevents all overlow possibilities.
        // Determine the ceiling of the desired number of nibbles
        let nibbles = (desired_size as u64 + 15) / 16;
        // Convert that to bytes
        let bytes = nibbles * 16;
        // Limit that to u32::MAX then convert to u32.
        let target = bytes.min(u32::MAX as u64) as u32;
        // The target has to be greater than the current_size or something is terribly wrong and
        // is going to get worse.
        assert!(target > current_size);
        // Return the new target.
        target
    }
}

pub struct GrowByQuarterKibi {}

impl GrowByQuarterKibi {
    pub fn new() -> Self {
        Self {}
    }
}

impl GrowStrategy for GrowByQuarterKibi {
    fn next_capacity(&self, _tries: usize, current_size: u32, desired_size: u32) -> u32 {
        // With desired_size a u32, doing the math with u64 prevents all overlow possibilities.
        // Determine the ceiling of the current number of quarter kibis plus some for alignment.
        let quarter_kibis = (desired_size as u64 + 255 + ALIGNMENT as u64) / 256;
        // Convert to bytes
        let bytes = quarter_kibis * 256;
        // Limit the target to a value that fits in a u32.
        let target = bytes.min(u32::MAX as u64) as u32;
        // The target has to be greater than the current_size or something is terribly wrong and
        // is going to get worse.
        assert!(target > current_size);
        target
    }
}

pub struct GrowByDoubleNibbles {
    floor: u64,
}

impl GrowByDoubleNibbles {
    pub fn new(floor: u32) -> Self {
        Self {
            floor: floor.into(),
        }
    }
}
impl GrowStrategy for GrowByDoubleNibbles {
    fn next_capacity(&self, _tries: usize, current_size: u32, desired_size: u32) -> u32 {
        // With current_size a u32, doing the math with u64 prevents all overlow possibilities.
        // Determine the ceiling of the current number of nibbles
        let nibbles = (current_size as u64 + 15) / 16;
        // Convert that to bytes doubled
        let doubled_bytes = nibbles * 16 * 2;
        // Use the largest of the doubled value, desired_size, or the preconfigured floor.
        // Limit that to u32::MAX.
        let target = doubled_bytes
            .max(desired_size as u64)
            .max(self.floor)
            .min(u32::MAX as u64) as u32;
        // The target has to be greater than the current_size or something is terribly wrong and
        // is going to get worse.
        assert!(target > current_size);
        // Return the new target.
        target
    }
}

pub struct GrowByBumpToNibble {}

impl GrowByBumpToNibble {
    pub fn new() -> Self {
        Self {}
    }
}

impl GrowStrategy for GrowByBumpToNibble {
    fn next_capacity(&self, _tries: usize, current_size: u32, desired_size: u32) -> u32 {
        // With desired_size a u32, doing the math with u64 prevents all overlow possibilities.
        // Determine the ceiling of the current number of nibbles after bumping to include space for
        // a NULL terminator (just in case of an API bug).
        let bumped_nibbles = (desired_size as u64 + SIZE_OF_WCHAR as u64 + 15) / 16;
        // Convert that to bytes
        let bytes = bumped_nibbles * 16;
        // Use the largest of the bumped nibbles value or the desired_size.
        // Limit that to u32::MAX.
        let target = bytes.max(desired_size as u64).min(u32::MAX as u64) as u32;
        // The target has to be greater than the current_size or something is terribly wrong and
        // is going to get worse.
        assert!(target > current_size);
        // Return the new target.
        target
    }
}
