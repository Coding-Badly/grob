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

use std::slice::from_raw_parts;

use windows::Win32::Foundation::FALSE;
use windows::Win32::NetworkManagement::IpHelper::{GetTcpTable2, MIB_TCPTABLE2};

use grob::{
    GrowToNearestQuarterKibi, GrowableBuffer, RvIsError, StackBuffer, ToResult, WriteBuffer,
};

fn common(initial_buffer: &mut dyn WriteBuffer) -> Result<(), Box<dyn std::error::Error>> {
    let grow_strategy = GrowToNearestQuarterKibi::new();

    // Loop until the call to GetTcpTable2 fails with an error or succeeds because the buffer has
    // enough space.
    let mut growable_buffer =
        GrowableBuffer::<MIB_TCPTABLE2, *mut MIB_TCPTABLE2>::new(initial_buffer, &grow_strategy);
    loop {
        // Prepare the argument for the API calll
        let mut argument = growable_buffer.argument();

        // Make the API call indicating what the return value means
        let rv = RvIsError::new(unsafe {
            GetTcpTable2(Some(argument.pointer()), argument.size(), FALSE)
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
        let number_of_entries: usize = unsafe { (*p).dwNumEntries }.try_into().unwrap();
        println!(
            "Number of entries in the returned data = {}",
            number_of_entries
        );
        let table = unsafe { from_raw_parts((*p).table.as_ptr(), number_of_entries) };
        for entry in table {
            println!("{}", entry.dwRemoteAddr);
        }
    }
    Ok(())
}

fn just_heap_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<0>::new();
    common(&mut initial_buffer)
}

fn start_with_stack_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let mut initial_buffer = StackBuffer::<8192>::new();
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
