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
