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
use windows::Win32::Networking::WinSock::AF_UNSPEC;

use grob::{winapi_large_binary, RvIsError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut names = winapi_large_binary(
        |argument| {
            RvIsError::new(unsafe {
                GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    GET_ADAPTERS_ADDRESSES_FLAGS(0),
                    None,
                    Some(argument.pointer()),
                    argument.size(),
                )
            })
        },
        |frozen_buffer| {
            let mut rv = Vec::new();
            if let Some(mut p) = frozen_buffer.pointer() {
                while p != std::ptr::null() {
                    rv.push(format!("{}", unsafe { (*p).FriendlyName.display() }));
                    p = unsafe { (*p).Next };
                }
            }
            Ok(rv)
        },
    )?;
    names.sort();
    println!("Names...");
    for name in names.into_iter() {
        println!("  {}", name);
    }
    Ok(())
}
