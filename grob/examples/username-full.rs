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
use windows::Win32::NetworkManagement::NetManagement::UNLEN;
use windows::Win32::System::WindowsProgramming::GetUserNameW;

use grob::{
    FillBufferAction, GrowStrategy, GrowableBuffer, RvIsError, StackBuffer, ToResult, WriteBuffer,
    ALIGNMENT, SIZE_OF_WCHAR,
};

const CAPACITY_FOR_NAMES: usize = ((UNLEN + 1) as usize * SIZE_OF_WCHAR as usize) + ALIGNMENT;

struct GrowByBumpToNibble {}

impl GrowByBumpToNibble {
    fn new() -> Self {
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
    // Our grow strategy is take what the operating system wants, bump a little to ensure there's
    // space for a NULL terminator then adjust to the nearest higher 16 byte boundary to try to
    // reduce heap fragmentation.
    let grow_strategy = GrowByBumpToNibble::new();

    // Loop until the call to GetUserNameW fails with an error or succeeds because the buffer has
    // enough space.
    let mut growable_buffer = GrowableBuffer::<u16, PWSTR>::new(initial_buffer, &grow_strategy);
    loop {
        // Prepare the argument for the API calll
        let mut argument = growable_buffer.argument();

        // Make the API call
        let rv = unsafe { GetUserNameW(argument.pointer(), argument.size()) };

        // Convert the return value to an error code
        let rv: RvIsError = rv.into();

        // Decide what to do next
        let fill_buffer_action = rv.to_result(&mut argument)?;

        // Apply the action
        match fill_buffer_action {
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
    // Do something with the returned data
    let frozen_buffer = growable_buffer.freeze();
    let username = frozen_buffer.to_string(true).unwrap();
    println!("GetUserNameW returned \"{}\"", username);

    Ok(())
}

fn just_heap_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<0>::new();
    common(&mut initial_buffer)
}

fn start_with_stack_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<CAPACITY_FOR_NAMES>::new();
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
