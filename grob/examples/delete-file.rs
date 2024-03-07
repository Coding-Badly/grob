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

use std::fs::{canonicalize, File};
use std::io::Write;

use windows::Win32::{Foundation::TRUE, Storage::FileSystem::DeleteFileW};

use grob::{AsPCWSTR, WindowsPathString};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let working_dir = canonicalize(".")?;
    let target_path = working_dir.join("delete-me.tmp");

    let mut output = File::create(&target_path)?;
    write!(output, "Please delete this file.")?;
    drop(output);

    let rv = unsafe { DeleteFileW(WindowsPathString::new(&target_path)?.as_param()) };
    if rv == TRUE {
        println!("{} successfully deleted.", target_path.display());
    } else {
        let loe = std::io::Error::last_os_error();
        println!("DeleteFileW failed.  The error is...\n  {:?}.", loe);
    }

    Ok(())
}
