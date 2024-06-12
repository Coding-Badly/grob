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

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::GetCurrentProcess;

use grob::{winapi_path_buf, RvIsSize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = winapi_path_buf(|argument| {
        RvIsSize::new(unsafe { GetModuleFileNameW(HMODULE(0), argument.as_mut_slice()) })
    })?;
    println!("GetModuleFileNameW returned {}", path.display());

    let path = winapi_path_buf(|argument| {
        RvIsSize::new(unsafe { GetModuleFileNameExW(GetCurrentProcess(), HMODULE(0), argument.as_mut_slice()) })
    })?;
    println!("GetModuleFileNameExW returned {}", path.display());

    Ok(())
}
