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

use windows::core::PWSTR;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

use grob::{
    FillBufferAction, GrowStrategy, GrowableBuffer, RvIsSize, StackBuffer, ToResult, WriteBuffer,
    ALIGNMENT, SIZE_OF_WCHAR,
};

const CAPACITY_FOR_PATHS: usize = (MAX_PATH as usize * SIZE_OF_WCHAR as usize) + ALIGNMENT;

struct GrowByDoubleNibbles {
    floor: u64,
}

impl GrowByDoubleNibbles {
    fn new(floor: u32) -> Self {
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
        // Output some stuff so the human knows what's happening.
        println!(
            "{:>2} {:>3} {:>3} {:>3}",
            _tries, current_size, desired_size, target
        );
        // Return the new target.
        target
    }
}

fn common(initial_buffer: &mut dyn WriteBuffer) -> Result<(), Box<dyn std::error::Error>> {
    // This example is meant to illustrate the difference between a well sized stack buffer and a
    // progressively larger heap buffer.  Using a floor would hide the comparison.
    let grow_strategy = GrowByDoubleNibbles::new(0);

    // Loop until the call to GetModuleFileNameW fails with an error or succeeds because the buffer
    // has enough space.
    let mut growable_buffer = GrowableBuffer::<u16, PWSTR>::new(initial_buffer, &grow_strategy);
    loop {
        let mut argument = growable_buffer.argument();
        let rv = unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) };
        let rv: RvIsSize = rv.into();
        let result = rv.to_result(&mut argument);
        // nfx: Move this code to an argument.appyly(result?) that returns true if we should break.
        match result? {
            FillBufferAction::Commit => {
                argument.commit();
                break;
            }
            FillBufferAction::Grow => {
                argument.grow();
            }
            FillBufferAction::NoData => {
                argument.commit_no_data();
                break;
            }
        }
    }
    let frozen_buffer = growable_buffer.freeze();
    let path = frozen_buffer.to_path_buf().unwrap();
    println!("GetModuleFileNameW returned \"{}\"", path.display());
    Ok(())
}

fn just_heap_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<0>::new();
    common(&mut initial_buffer)
}

fn start_with_stack_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<CAPACITY_FOR_PATHS>::new();
    common(&mut initial_buffer)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();

    start_with_stack_buffer()?;
    println!();

    just_heap_buffer()?;
    println!();

    println!();
    Ok(())
}
