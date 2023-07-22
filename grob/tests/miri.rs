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

mod large_binary {
    mod rv_is_error {
        use windows::Win32::Foundation::{ERROR_ADDRESS_NOT_ASSOCIATED, ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};

        use grob::{winapi_large_binary, RvIsError};

        fn write_zero_bytes(_data: Option<*mut u8>, size: *mut u32) -> u32 {
            unsafe { *size = 0 };
            ERROR_SUCCESS.0
        }

        #[test]
        fn nothing_stored() {
            winapi_large_binary(
                |argument| {
                    RvIsError::new(write_zero_bytes(Some(argument.pointer()), argument.size()))
                },
                |frozen_buffer| {
                    assert!(frozen_buffer.size() == 0);
                    Ok(())
                },
            ).unwrap();
        }

        fn write_one_byte(data: Option<*mut u8>, size: *mut u32) -> u32 {
            let rv = if unsafe { *size } > 0 {
                unsafe { *(data.unwrap()) = 42 };
                ERROR_SUCCESS.0
            } else {
                ERROR_BUFFER_OVERFLOW.0
            };
            unsafe { *size = 1 };
            rv
        }

        #[test]
        fn one_byte_stored() {
            winapi_large_binary(
                |argument| {
                    RvIsError::new(write_one_byte(Some(argument.pointer()), argument.size()))
                },
                |frozen_buffer| {
                    assert!(frozen_buffer.size() == 1);
                    let p = frozen_buffer.pointer().unwrap();
                    assert!(p != std::ptr::null());
                    assert!(unsafe { *p } == 42);
                    Ok(())
                },
            ).unwrap();
        }

        fn grow_then_fill(tries: usize, data: Option<*mut u8>, size: *mut u32) -> u32 {
            if tries == 1 {
                unsafe { *size += 1; }
                ERROR_BUFFER_OVERFLOW.0
            } else {
                let p = data.unwrap();
                assert!(p != std::ptr::null_mut());
                unsafe { std::ptr::write_bytes(p, 42, (*size).try_into().unwrap()) };
                ERROR_SUCCESS.0
            }
        }

        #[test]
        fn full_stack_buffer() {
            winapi_large_binary(
                |argument| {
                    RvIsError::new(grow_then_fill(argument.tries(), Some(argument.pointer()), argument.size()))
                },
                |frozen_buffer| {
                    assert!(frozen_buffer.size() > 0);
                    let p = frozen_buffer.pointer().unwrap();
                    assert!(p != std::ptr::null());
                    let s = unsafe { std::slice::from_raw_parts(p, frozen_buffer.size().try_into().unwrap()) };
                    for v in s.into_iter() {
                        assert!(*v == 42);
                    }
                    Ok(())
                },
            ).unwrap();
        }

        fn return_error(_tries: usize, _data: Option<*mut u8>, _size: *mut u32) -> u32 {
            ERROR_ADDRESS_NOT_ASSOCIATED.0
        }

        #[test]
        fn no_finalize_when_error() {
            match winapi_large_binary(
                |argument| {
                    RvIsError::new(return_error(argument.tries(), Some(argument.pointer()), argument.size()))
                },
                |_frozen_buffer| {
                    assert!(false);
                    Ok(())
                },
            ) {
                Ok(()) => assert!(false),
                Err(_e) => {
                    if let Some(raw) = _e.raw_os_error() {
                        if raw == ERROR_ADDRESS_NOT_ASSOCIATED.0 as i32 {
                            // all good
                        } else {
                            assert!(false);
                        }
                    } else {
                        assert!(false);
                    }
                }
            }
        }
    }
}

mod small_binary {
    mod rv_is_size {
        use std::mem::size_of;

        use windows::Win32::Foundation::{ERROR_ADDRESS_NOT_ASSOCIATED, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, SetLastError};

        use grob::{winapi_small_binary, RvIsSize};

        const LARGE_INTEGER: u128 = 12345678901234567890123456789012345678_u128;

        fn write_zero_bytes(_data: Option<*mut u128>, _size: u32) -> u32 {
            unsafe { SetLastError(ERROR_SUCCESS) };
            0
        }

        #[test]
        fn nothing_stored() {
            winapi_small_binary(
                |argument| {
                    RvIsSize::new(write_zero_bytes(Some(argument.pointer()), unsafe { *argument.size() } ))
                },
                |frozen_buffer| {
                    assert!(frozen_buffer.size() == 0);
                    Ok(())
                },
            ).unwrap();
        }

        fn write_one_thing(data: Option<*mut u128>, size: *mut u32) -> u32 {
            if unsafe { *size } > 0 {
                unsafe { *(data.unwrap()) = LARGE_INTEGER };
            }
            size_of::<u128>().try_into().unwrap()
        }

        #[test]
        fn one_thing_stored() {
            winapi_small_binary(
                |argument| {
                    RvIsSize::new(write_one_thing(Some(argument.pointer()), argument.size()))
                },
                |frozen_buffer| {
                    assert!(frozen_buffer.size() == 16);
                    let p = frozen_buffer.pointer().unwrap();
                    assert!(p != std::ptr::null());
                    assert!(unsafe { *p } == LARGE_INTEGER);
                    Ok(())
                },
            ).unwrap();
        }

        fn grow_then_fill(tries: usize, data: Option<*mut u128>, size: u32) -> u32 {
            if tries == 1 {
                unsafe { SetLastError(ERROR_INSUFFICIENT_BUFFER) };
                size
            } else {
                let c = size as usize / size_of::<u128>();
                let p = data.unwrap();
                let s = std::ptr::slice_from_raw_parts_mut(p, c);
                for e in unsafe { (*s).iter_mut() } {
                    *e = LARGE_INTEGER;
                }
                unsafe { SetLastError(ERROR_SUCCESS) };
                size - 1
            }
        }

        #[test]
        fn full_stack_buffer() {
            winapi_small_binary(
                |argument| {
                    RvIsSize::new(grow_then_fill(argument.tries(), Some(argument.pointer()), unsafe{*argument.size()}))
                },
                |frozen_buffer| {
                    assert!(frozen_buffer.size() > 0);
                    let p = frozen_buffer.pointer().unwrap();
                    assert!(p != std::ptr::null());
                    assert!(unsafe{*p} == LARGE_INTEGER);
                    let last = ((frozen_buffer.size() as usize / size_of::<u128>()) - 1) as isize;
                    assert!(unsafe{*(p.offset(last))} == LARGE_INTEGER);
                    Ok(())
                },
            ).unwrap();
        }

        fn return_error(_tries: usize, _data: Option<*mut u8>, _size: u32) -> u32 {
            unsafe { SetLastError(ERROR_ADDRESS_NOT_ASSOCIATED) };
            0
        }

        #[test]
        fn no_finalize_when_error() {
            match winapi_small_binary(
                |argument| {
                    RvIsSize::new(return_error(argument.tries(), Some(argument.pointer()), unsafe { *argument.size() }))
                },
                |_frozen_buffer| {
                    assert!(false);
                    Ok(())
                },
            ) {
                Ok(()) => assert!(false),
                Err(_e) => {
                    if let Some(raw) = _e.raw_os_error() {
                        if raw == ERROR_ADDRESS_NOT_ASSOCIATED.0 as i32 {
                            // all good
                        } else {
                            assert!(false);
                        }
                    } else {
                        assert!(false);
                    }
                }
            }
        }
    }
}

mod string {
    mod rv_is_error {
        use std::os::windows::ffi::OsStrExt;

        use windows::core::PWSTR;
        use windows::Win32::Foundation::{BOOL, ERROR_INSUFFICIENT_BUFFER, FALSE, TRUE, SetLastError};

        use grob::{winapi_string, RvIsError};

        fn write_zero_bytes(_data: PWSTR, size: *mut u32) -> BOOL {
            unsafe { *size = 0 };
            TRUE
        }

        #[test]
        fn nothing_stored() {
            let s = winapi_string(false,
                |argument| {
                    RvIsError::new(write_zero_bytes(argument.pointer(), argument.size()))
                }
            ).unwrap().unwrap();
            assert!(s == "");
        }

        fn write_terminator(data: PWSTR, size: *mut u32) -> BOOL {
            let rv = if unsafe { *size > 0 } {
                unsafe { *data.0 = 0 };
                TRUE
            } else {
                unsafe { SetLastError(ERROR_INSUFFICIENT_BUFFER) };
                FALSE
            };
            unsafe { *size = 1 };
            rv
        }

        #[test]
        fn terminator_stored() {
            let s = winapi_string(false,
                |argument| {
                    RvIsError::new(write_terminator(argument.pointer(), argument.size()))
                }
            ).unwrap().unwrap();
            assert!(s == "");
        }

        const ZATHRAS: [u16; 8] = ['Z' as u16, 'a' as u16, 't' as u16, 'h' as u16, 'r' as u16, 'a' as u16, 's' as u16, 0];

        fn write_zathras(data: PWSTR, size: *mut u32) -> BOOL {
            let rv = if unsafe { *size >= ZATHRAS.len() as u32} {
                unsafe { std::ptr::copy(ZATHRAS.as_ptr(), data.0, ZATHRAS.len()) };
                TRUE
            } else {
                unsafe { SetLastError(ERROR_INSUFFICIENT_BUFFER) };
                FALSE
            };
            unsafe { *size = ZATHRAS.len() as u32 };
            rv
        }

        #[test]
        fn try_zathras() {
            let s = winapi_string(false,
                |argument| {
                    RvIsError::new(write_zathras(argument.pointer(), argument.size()))
                }
            ).unwrap().unwrap();
            assert!(s == "Zathras");
        }

        const INVALID_UNICODE: [u16; 4] = ['a' as u16, 0xD800, 'z' as u16, 0];

        fn write_invalid_unicode(data: PWSTR, size: *mut u32) -> BOOL {
            let rv = if unsafe { *size >= INVALID_UNICODE.len() as u32} {
                unsafe { std::ptr::copy(INVALID_UNICODE.as_ptr(), data.0, INVALID_UNICODE.len()) };
                TRUE
            } else {
                unsafe { SetLastError(ERROR_INSUFFICIENT_BUFFER) };
                FALSE
            };
            unsafe { *size = INVALID_UNICODE.len() as u32 };
            rv
        }

        #[test]
        fn invalid_unicode_dropped() {
            let s = winapi_string(true,
                |argument| {
                    RvIsError::new(write_invalid_unicode(argument.pointer(), argument.size()))
                }
            ).unwrap().unwrap();
            // Rust replaces invalid UTF things with the Unicode Replacement Character U+FFFD.
            let c = "a\u{FFFD}z";
            assert!(s == c);
        }

        #[test]
        fn invalid_unicode_fails() {
            let rv = winapi_string(false,
                |argument| {
                    RvIsError::new(write_invalid_unicode(argument.pointer(), argument.size()))
                }
            ).unwrap();
            match rv {
                Ok(_) => assert!(false),
                Err(s) => {
                    // Convert the string
                    let r: Vec<u16> = s.encode_wide().collect();
                    // Compare the two.  r should be one byte shorter (no terminator) so just the
                    // actual characters will end up being compared.
                    let e = r.into_iter().zip(INVALID_UNICODE).fold(true, |a, v| a && (v.0 == v.1));
                    assert!(e);
                }
            }
        }
    }
}

mod path_buf {
    mod rv_is_size {
        use windows::Win32::Foundation::{ERROR_SUCCESS, SetLastError};

        use grob::{winapi_path_buf, RvIsSize};

        fn write_zero_bytes(_buffer: &mut [u16]) -> u32 {
            unsafe { SetLastError(ERROR_SUCCESS) };
            0
        }

        #[test]
        fn nothing_stored() {
            let path = winapi_path_buf(
                |argument| {
                    RvIsSize::new(write_zero_bytes(argument.as_mut_slice()))
                }
            ).unwrap();
            assert!(path.as_os_str() == "");
        }

        fn write_path(buffer: &mut [u16]) -> u32 {
            buffer[0] = 'C' as u16;
            buffer[1] = ':' as u16;
            buffer[2] = '\\' as u16;
            buffer[3] = 'W' as u16;
            buffer[4] = 'h' as u16;
            buffer[5] = 'a' as u16;
            buffer[6] = 't' as u16;
            buffer[7] = 'e' as u16;
            buffer[8] = 'v' as u16;
            buffer[9] = 'e' as u16;
            buffer[10] = 'r' as u16;
            buffer[11] = '\\' as u16;
            buffer[12] = 'a' as u16;
            buffer[13] = '\\' as u16;
            buffer[14] = 'b' as u16;
            buffer[15] = '\\' as u16;
            buffer[16] = 'c' as u16;
            buffer[17] = '\\' as u16;
            buffer[18] = 'd' as u16;
            buffer[19] = '.' as u16;
            buffer[20] = 't' as u16;
            buffer[21] = 'x' as u16;
            buffer[22] = 't' as u16;
            buffer[23] = 0;
            unsafe { SetLastError(ERROR_SUCCESS) };
            24
        }

        #[test]
        fn whatever_stored() {
            let path = winapi_path_buf(
                |argument| {
                    RvIsSize::new(write_path(argument.as_mut_slice()))
                }
            ).unwrap();
            let s = path.as_os_str();
            assert!(s == "C:\\Whatever\\a\\b\\c\\d.txt");
            assert!(s.len() == 23);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
