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

use grob::{winapi_small_binary, RvIsError};

use windows::Win32::System::SystemInformation::{GetLogicalProcessorInformationEx, RelationGroup};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mpc = winapi_small_binary(
        |argument| {
            RvIsError::new(unsafe {
                GetLogicalProcessorInformationEx(
                    RelationGroup,
                    Some(argument.pointer()),
                    argument.size(),
                )
            })
        },
        |frozen_buffer| {
            if let Some(p) = frozen_buffer.pointer() {
                let r = unsafe { (*p).Relationship };
                let mpc = unsafe { (*p).Anonymous.Group.GroupInfo[0].MaximumProcessorCount };
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
                println!("MaximumProcessorCount = {:?}", mpc);
                println!();
                Ok(Some(mpc))
            } else {
                Ok(None)
            }
        },
    )?;

    if let Some(mpc) = mpc {
        println!("Success!  The maximum processor count is {}.", mpc);
    }
    Ok(())
}
