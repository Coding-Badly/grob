// Copyright 2024 Brian Cook (a.k.a. Coding-Badly)
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

use windows::Win32::Foundation::TRUE;
use windows::Win32::System::SystemInformation::SetComputerNameW;
use windows::Win32::System::WindowsProgramming::{GetComputerNameW, MAX_COMPUTERNAME_LENGTH};

use grob::{winapi_string, AsPCWSTR, RvIsError, WindowsString};

const BETTER_MAX_COMPUTERNAME_LENGTH: usize = MAX_COMPUTERNAME_LENGTH as usize;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();

    // Get the current computer name so it can be restored
    let original_name = winapi_string(true, |argument| {
        RvIsError::new(unsafe { GetComputerNameW(argument.pointer(), argument.size()) })
    })?
    .unwrap();
    println!("GetComputerNameW returned {}", original_name);
    println!();

    // Change the name to something temporary
    let new_name = WindowsString::<BETTER_MAX_COMPUTERNAME_LENGTH>::new("TEMPNAME")?;
    let rv = unsafe { SetComputerNameW(new_name.as_param()) };
    if rv == TRUE {
        println!("Changing the computer name was successful.");
        println!();
    } else {
        let loe = std::io::Error::last_os_error();
        println!(
            "Changing the computer name failed.  The error is...\n  {:?}.",
            loe
        );
        println!();
    }

    // Restore the computer name
    let rv = unsafe {
        SetComputerNameW(
            WindowsString::<BETTER_MAX_COMPUTERNAME_LENGTH>::new(&original_name)?.as_param(),
        )
    };
    if rv == TRUE {
        println!("Restoring the computer name was successful.");
        println!();
    } else {
        let loe = std::io::Error::last_os_error();
        println!(
            "Restoring the computer name failed.  The error is...\n  {:?}.",
            loe
        );
        println!();
    }

    println!();
    Ok(())
}
