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

use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformationEx, RelationGroup, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};

use grob::{GrowStrategy, GrowableBuffer, RvIsError, StackBuffer, ToResult, WriteBuffer};

struct GrowByNearestNibble {}

impl GrowByNearestNibble {
    fn new() -> Self {
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
    let grow_strategy = GrowByNearestNibble::new();

    // Loop until the call to GetAdaptersAddresses fails with an error or succeeds because the
    // buffer has enough space.
    let mut growable_buffer = GrowableBuffer::<
        SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
        *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
    >::new(initial_buffer, &grow_strategy);
    loop {
        // Prepare the argument for the API calll
        let mut argument = growable_buffer.argument();

        // Make the API call indicating what the return value means
        let rv = RvIsError::new(unsafe {
            GetLogicalProcessorInformationEx(
                RelationGroup,
                Some(argument.pointer()),
                argument.size(),
            )
        });

        // Convert the return value to an action
        let fill_buffer_action = rv.to_result(&mut argument)?;

        // Apply the action
        if argument.apply(fill_buffer_action) {
            break;
        }
    }
    // Do something with the data
    let frozen_buffer = growable_buffer.freeze();
    if let Some(p) = frozen_buffer.pointer() {
        let r = unsafe { (*p).Relationship };
        println!("Relationship = {:?}", r); // Has to be RelationGroup
        println!("Size = {:?}", unsafe { (*p).Size });
        println!("MaximumGroupCount = {:?}", unsafe {
            (*p).Anonymous.Group.MaximumGroupCount
        });
        println!("ActiveGroupCount = {:?}", unsafe {
            (*p).Anonymous.Group.ActiveGroupCount
        });
        println!("ActiveProcessorCount = {:?}", unsafe {
            (*p).Anonymous.Group.GroupInfo[0].ActiveProcessorCount
        });
        println!("MaximumProcessorCount = {:?}", unsafe {
            (*p).Anonymous.Group.GroupInfo[0].MaximumProcessorCount
        });
        println!();
    }
    Ok(())
}

fn just_heap_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<0>::new();
    common(&mut initial_buffer)
}

fn start_with_stack_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<1024>::new();
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
