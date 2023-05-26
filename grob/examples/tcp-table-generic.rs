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
use windows::Win32::NetworkManagement::IpHelper::GetTcpTable2;

use grob::{winapi_large_binary, RvIsError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    winapi_large_binary(
        |argument| {
            RvIsError::new(unsafe {
                GetTcpTable2(Some(argument.pointer()), argument.size(), FALSE)
            })
        },
        |frozen_buffer| {
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
        },
    )?;
    Ok(())
}
