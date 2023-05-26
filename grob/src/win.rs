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

use std::ffi::OsString;
use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::slice::{from_raw_parts, from_raw_parts_mut};

use windows::core::PWSTR;
use windows::Win32::Foundation::{
    GetLastError, SetLastError, BOOL, ERROR_BUFFER_OVERFLOW, ERROR_INSUFFICIENT_BUFFER,
    ERROR_NO_DATA, MAX_PATH, NO_ERROR, TRUE, WIN32_ERROR,
};
use windows::Win32::NetworkManagement::NetManagement::UNLEN;

use crate::base::{FillBufferAction, FillBufferResult};
use crate::buffer::os::ALIGNMENT;
use crate::traits::{NeededSize, RawToInternal, ToResult};
use crate::{Argument, FrozenBuffer};

pub const SIZE_OF_WCHAR: u32 = size_of::<u16>() as u32;
pub const CAPACITY_FOR_NAMES: usize = ((UNLEN + 1) as usize * SIZE_OF_WCHAR as usize) + ALIGNMENT;
pub const CAPACITY_FOR_PATHS: usize = (MAX_PATH as usize * SIZE_OF_WCHAR as usize) + ALIGNMENT;

impl<'gb> Argument<'gb, PWSTR> {
    pub fn as_mut_slice(&mut self) -> &mut [u16] {
        let rv = unsafe { from_raw_parts_mut(self.pointer.0, self.size as usize) };
        unsafe { SetLastError(NO_ERROR) };
        rv
    }
}

#[derive(Debug)]
pub struct RvIsError(WIN32_ERROR);

impl RvIsError {
    pub fn new<T>(value: T) -> Self
    where
        T: Into<Self>,
    {
        value.into()
    }
}

impl ToResult for RvIsError {
    fn to_result(&self, needed_size: &mut dyn NeededSize) -> FillBufferResult {
        let rv = match self.0 {
            NO_ERROR => Ok(FillBufferAction::Commit),
            ERROR_INSUFFICIENT_BUFFER => Ok(FillBufferAction::Grow),
            ERROR_BUFFER_OVERFLOW => Ok(FillBufferAction::Grow),
            ERROR_NO_DATA => Ok(FillBufferAction::NoData),
            c @ _ => Err(std::io::Error::from_raw_os_error(c.0 as i32)),
        };
        if rv.is_ok() && needed_size.needed_size() == 0 {
            Ok(FillBufferAction::NoData)
        } else {
            rv
        }
    }
}

impl From<BOOL> for RvIsError {
    fn from(value: BOOL) -> Self {
        if value == TRUE {
            Self(NO_ERROR)
        } else {
            Self(unsafe { GetLastError() })
        }
    }
}

impl From<u32> for RvIsError {
    fn from(value: u32) -> Self {
        Self(WIN32_ERROR(value))
    }
}

#[derive(Debug)]
pub struct RvIsSize(u32, WIN32_ERROR);

impl RvIsSize {
    pub fn new<T>(value: T) -> Self
    where
        T: Into<Self>,
    {
        value.into()
    }
}

impl ToResult for RvIsSize {
    fn to_result(&self, needed_size: &mut dyn NeededSize) -> FillBufferResult {
        let ns = needed_size.needed_size();
        // Either an error or success with nothing stored
        if self.0 == 0 {
            // Success with nothing stored
            if self.1 == NO_ERROR {
                Ok(FillBufferAction::NoData)
            // The buffer has no capacity.  Very likely because the caller does not want to use a
            // stack buffer.  The expectation is that the GrowStrategy will have a reasonable
            // minimum capacity so we'll just indicate something more than zero.
            } else if ns == 0 {
                needed_size.set_needed_size(1);
                Ok(FillBufferAction::Grow)
            // Error
            } else {
                Err(std::io::Error::from_raw_os_error(self.1 .0 as i32))
            }
        // Buffer was big enough.  self.1 is presumed to be NO_ERROR.
        } else if self.0 < ns {
            needed_size.set_needed_size(self.0);
            Ok(FillBufferAction::Commit)
        // Buffer does not have space for the terminator.
        } else if self.1 == ERROR_INSUFFICIENT_BUFFER {
            needed_size.set_needed_size(self.0.saturating_mul(2));
            Ok(FillBufferAction::Grow)
        // At this point the API function returned precisely the buffer capacity and set the last
        // error to something other than ERROR_INSUFFICIENT_BUFFER.  Or, the API function returned a
        // value greater than the capacity.  Those are both undocument behaviour.
        } else {
            unreachable!()
        }
    }
}

impl From<u32> for RvIsSize {
    fn from(value: u32) -> Self {
        let gle = unsafe { GetLastError() };
        Self(value, gle)
    }
}

impl RawToInternal for PWSTR {
    fn capacity_to_size(value: u32) -> u32 {
        // The size is specified in WCHARs.
        value / crate::SIZE_OF_WCHAR
    }
    fn convert_pointer(value: *mut u8) -> PWSTR {
        PWSTR(value as *mut u16)
    }
    fn size_to_capacity(value: u32) -> u32 {
        // The size is specified in WCHARs.
        value.saturating_mul(crate::SIZE_OF_WCHAR)
    }
}

impl<'sb> FrozenBuffer<'sb, u16> {
    pub fn to_path_buf(&self) -> Option<PathBuf> {
        self.to_os_string().map(|v| PathBuf::from(v))
    }
    pub fn to_os_string(&self) -> Option<OsString> {
        let (p, s) = self.read_buffer();
        if s == 0 {
            return None;
        }
        assert!(s > 0);
        if let Some(p) = p {
            let v = unsafe { from_raw_parts(p, s as usize) };
            // Protected by the "s == 0" check and assert above.
            let last: usize = if *v.last().unwrap() == 0 { s - 1 } else { s }
                .try_into()
                .unwrap();
            Some(OsString::from_wide(&v[..last]))
        } else {
            None
        }
    }
    pub fn to_string(&self, lossy_ok: bool) -> Result<String, OsString> {
        match self.to_os_string() {
            Some(s) => {
                if lossy_ok {
                    Ok(s.to_string_lossy().to_string())
                } else {
                    s.into_string()
                }
            }
            None => Ok(String::new()),
        }
    }
}
