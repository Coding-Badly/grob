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

use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, FALSE};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO};
use windows::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
 
use grob::{winapi_large_binary, winapi_path_buf, RvIsError, RvIsSize};

struct ApiString(Vec<u16>);

impl ApiString {
    fn ffi(&self) -> PCWSTR {
        PCWSTR::from_raw(self.0.as_ptr())
    }
}

impl From<&Path> for ApiString {
    fn from(value: &Path) -> Self {
        Self(value.as_os_str().encode_wide().chain(Some(0)).collect())
    }
}

impl From<&str> for ApiString {
    fn from(value: &str) -> Self {
        Self(OsStr::new(value).encode_wide().chain(Some(0)).collect())
    }
}

#[derive(Debug)]
struct PeVersion {
    major: u16,
    minor: u16,
    patch: u16,
    build: u16,
}

impl Display for PeVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let txt = format!("{}.{}.{}.{}", self.major, self.minor, self.patch, self.build);
        f.pad(&txt)
    }
}

impl From<(u32, u32)> for PeVersion {
    fn from(value:(u32, u32)) -> Self {
        Self {
            major: ((value.0 >> 16) & 0xFFFF) as u16,
            minor: ((value.0 >> 0) & 0xFFFF) as u16,
            patch: ((value.1 >> 16) & 0xFFFF) as u16,
            build: ((value.1 >> 0) & 0xFFFF) as u16,
        }
    }
}

impl From<*const VS_FIXEDFILEINFO> for PeVersion {
    fn from(value: *const VS_FIXEDFILEINFO) -> PeVersion {
        assert!(unsafe { (*(value as *const VS_FIXEDFILEINFO)).dwSignature } == 0xFEEF04BD);
        (
            unsafe { (*(value as *const VS_FIXEDFILEINFO)).dwProductVersionMS },
            unsafe { (*(value as *const VS_FIXEDFILEINFO)).dwProductVersionLS }
        ).into()
    }
}

fn get_pe_version<P>(path: P) -> Result<Option<PeVersion>, std::io::Error>
where
    P: AsRef<Path>,
{
    winapi_large_binary(
        |argument| {
            let a: ApiString = path.as_ref().into();
            let needed = unsafe{GetFileVersionInfoSizeW(a.ffi(), None)};
            if needed == 0 {
                return RvIsError::new(FALSE);
            }
            let s = unsafe { *argument.size() };
            if s < needed {
                unsafe { *argument.size() = needed };
                return RvIsError::new(ERROR_INSUFFICIENT_BUFFER.0);
            }
            RvIsError::new(unsafe{GetFileVersionInfoW(a.ffi(), 0, s, argument.pointer())})
        },
        |frozen_buffer| {
            if let Some(p) = frozen_buffer.pointer() {
                let a: ApiString = "\\".into();
                let mut l: u32 = 0;
                let mut d: *mut std::ffi::c_void = std::ptr::null_mut();
                let rv = unsafe { VerQueryValueW(p, a.ffi(), &mut d, &mut l) };
                if rv == FALSE {
                    return Err(std::io::Error::last_os_error());
                }
                let pev: PeVersion = (d as *const VS_FIXEDFILEINFO).into();
                Ok(Some(pev))
            } else {
                Ok(None)
            }
        }
    )
}

fn recurse(path: PathBuf) {
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd {
            match e {
                Ok(e) => {
                    if let Ok(ft) = e.file_type() {
                        let p = e.path();
                        if ft.is_dir() {
                            if p.ancestors().count() < 4 {
                                recurse(p);
                            }
                        } else {
                            if let Ok(Some(pev)) = get_pe_version(&p) {
                                println!("{:<18} {:?}", pev, e.file_name());
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
}

fn get_windows_system_dir() -> Result<PathBuf, std::io::Error> {
    winapi_path_buf(|argument| {
        RvIsSize::new(unsafe { GetSystemWindowsDirectoryW(Some(argument.as_mut_slice())) })
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();

    let sys_dir = get_windows_system_dir()?;

    recurse(sys_dir);

    println!();
    Ok(())
}
