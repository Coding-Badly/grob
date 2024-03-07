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

use std::fs::{canonicalize, read_to_string, remove_file, File};
use std::io::Write;

use windows::Win32::Foundation::TRUE;
use windows::Win32::Storage::FileSystem::{ReplaceFileW, REPLACE_FILE_FLAGS};

use grob::{AsPCWSTR, WindowsPathString};

const TARGET: &str =
    "This is a test.  This is only a test.  If the test works, this will be overwritten.";
const SOURCE: &str =
    "This is a test.  This is only a test.  If the test works, this will be the new contents.";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();

    // Prepare full paths to the target and source
    let working_dir = canonicalize(".")?;
    let target_path = working_dir.join("target.tmp");
    let source_path = working_dir.join("source.tmp");
    let backup_path = working_dir.join("backup.tmp");

    // Create the target file
    let mut output = File::create(&target_path)?;
    write!(output, "{}", TARGET)?;
    drop(output);

    // Create the source file
    let mut output = File::create(&source_path)?;
    write!(output, "{}", SOURCE)?;
    drop(output);

    // Replace the target with the source
    // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/FileSystem/fn.ReplaceFileW.html

    let rv = unsafe {
        ReplaceFileW(
            WindowsPathString::new(&target_path)?.as_param(),
            WindowsPathString::new(&source_path)?.as_param(),
            WindowsPathString::new(&backup_path)?.as_param(),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    };

    // If ReplaceFileW indicated success
    if rv == TRUE {
        println!("ReplaceFileW returned success.");
        println!();

        // Ensure the source file is gone (it should now be named "target.tmp")
        if !source_path.exists() {
            println!(
                "The source file, {}, does not exist as expected.",
                source_path.display()
            );
        } else {
            println!(
                "The source file, {}, still exists.  Something has gone wrong.",
                source_path.display()
            );
        }
        println!();

        // Ensure the contents of the target are correct
        let t = read_to_string(&target_path)?;
        if t == SOURCE {
            println!(
                "The contents of the source file, {}, replaced the target file, {}, as expected.",
                source_path.display(),
                target_path.display()
            );
        } else {
            println!(
                "The target file was supposed to contain...\n  {}\n...but instead contains...\n  {}\nSomething has gone wrong.",
                SOURCE,
                t);
        }
        println!();

        // Ensure the contents of the backup are correct
        let t = read_to_string(&backup_path)?;
        if t == TARGET {
            println!(
                "The contents of the backup file, {}, are correct.",
                backup_path.display()
            );
        } else {
            println!(
                "The backup file was supposed to contain...\n  {}\n...but instead contains...\n  {}\nSomething has gone wrong.",
                TARGET,
                t);
        }
        println!();
    } else {
        let loe = std::io::Error::last_os_error();
        println!("ReplaceFileW failed.  The error is...\n  {:?}.", loe);
    }

    // Remove the three files regardless of the outcome (clean up our mess)
    println!("Cleaning up.");
    let _ = remove_file(&target_path);
    let _ = remove_file(&source_path);
    let _ = remove_file(&backup_path);

    println!();
    Ok(())
}
