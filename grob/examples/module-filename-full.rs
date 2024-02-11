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
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

use grob::{
    FillBufferAction, GrowForStoredIsReturned, GrowStrategy, GrowableBuffer, RvIsSize, StackBuffer,
    ToResult, WriteBuffer, CAPACITY_FOR_PATHS,
};

struct PrintNextCapacity {
    wrapped: Box<dyn GrowStrategy>,
}

impl PrintNextCapacity {
    fn new<GS>(wrapped: GS) -> Self
    where
        GS: GrowStrategy + 'static,
    {
        let wrapped = Box::new(wrapped);
        Self { wrapped }
    }
}

impl GrowStrategy for PrintNextCapacity {
    fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32 {
        let rv = self.wrapped.next_capacity(tries, desired_capacity);
        println!(
            "next_capacity(tries={}, desired_capacity={}) = {}",
            tries, desired_capacity, rv
        );
        rv
    }
}

fn common(initial_buffer: &mut dyn WriteBuffer) -> Result<(), Box<dyn std::error::Error>> {
    // This example is meant to illustrate the difference between a well sized stack buffer and a
    // progressively larger heap buffer.  Using a floor would hide the comparison.
    let grow_strategy = PrintNextCapacity::new(GrowForStoredIsReturned::<0>::new());

    // Loop until the call to GetModuleFileNameW fails with an error or succeeds because the buffer
    // has enough space.
    let mut growable_buffer = GrowableBuffer::<u16, PWSTR>::new(initial_buffer, &grow_strategy);
    loop {
        let mut argument = growable_buffer.argument();
        let rv = unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) };
        let rv: RvIsSize = rv.into();
        let result = rv.to_result(&mut argument);
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

    println!("Try with a reasonably sized stack buffer...");
    start_with_stack_buffer()?;
    println!();

    println!("Try with a zero sized stack buffer and no floor...");
    just_heap_buffer()?;
    println!();

    println!();
    Ok(())
}
