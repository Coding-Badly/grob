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

use windows::Win32::NetworkManagement::IpHelper::GetAdaptersAddresses;
use windows::Win32::NetworkManagement::IpHelper::GET_ADAPTERS_ADDRESSES_FLAGS;
use windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH;
use windows::Win32::Networking::WinSock::AF_UNSPEC;

use grob::{
    GrowStrategy, GrowableBuffer, RvIsError, StackBuffer, ToResult, WriteBuffer, ALIGNMENT,
};

struct GrowByQuarterKibi {}

impl GrowByQuarterKibi {
    fn new() -> Self {
        Self {}
    }
}

impl GrowStrategy for GrowByQuarterKibi {
    fn next_capacity(&self, tries: usize, current_size: u32, desired_size: u32) -> u32 {
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
        // Output some stuff so the human knows what's happening.
        println!(
            "{:>2} {:>3} {:>3} {:>3}",
            tries, current_size, desired_size, target
        );
        target
    }
}

fn common(initial_buffer: &mut dyn WriteBuffer) -> Result<(), Box<dyn std::error::Error>> {
    let grow_strategy = GrowByQuarterKibi::new();

    // Loop until the call to GetAdaptersAddresses fails with an error or succeeds because the
    // buffer has enough space.
    let mut growable_buffer =
        GrowableBuffer::<IP_ADAPTER_ADDRESSES_LH, *mut IP_ADAPTER_ADDRESSES_LH>::new(
            initial_buffer,
            &grow_strategy,
        );
    loop {
        // Prepare the argument for the API calll
        let mut argument = growable_buffer.argument();

        // Make the API call indicating what the return value means
        let rv = RvIsError::new(unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GET_ADAPTERS_ADDRESSES_FLAGS(0),
                None,
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
    let p = frozen_buffer.pointer();
    println!("pointer = {:?}, size = {}", p, frozen_buffer.size());
    if let Some(mut p) = p {
        while p != std::ptr::null() {
            println!("FriendlyName = {}", unsafe { (*p).FriendlyName.display() });
            p = unsafe { (*p).Next };
        }
    }
    println!();

    Ok(())
}

fn just_heap_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<0>::new();
    common(&mut initial_buffer)
}

fn start_with_stack_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<65536>::new();
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
