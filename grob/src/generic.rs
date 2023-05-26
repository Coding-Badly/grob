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

use windows::core::PWSTR;

use crate::buffer::StackBuffer;
use crate::strategy::{
    GrowByBumpToNibble, GrowByDoubleNibbles, GrowByNearestNibble, GrowByQuarterKibi,
};
use crate::traits::{GrowStrategy, RawToInternal, ToResult, WriteBuffer};
use crate::win::{CAPACITY_FOR_NAMES, CAPACITY_FOR_PATHS};
use crate::{Argument, FrozenBuffer, GrowableBuffer};

pub fn winapi_generic<FT, IT, W, WR, F, U>(
    mut growable_buffer: GrowableBuffer<FT, IT>,
    mut api_wrapper: W,
    mut finalize: F,
) -> Result<U, std::io::Error>
where
    IT: RawToInternal,
    IT: Copy,
    WR: ToResult,
    W: FnMut(&mut Argument<IT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    loop {
        let mut argument = growable_buffer.argument();
        let rv = api_wrapper(&mut argument);
        let fill_buffer_action = rv.to_result(&mut argument)?;
        if argument.apply(fill_buffer_action) {
            break;
        }
    }
    finalize(growable_buffer.freeze())
}

pub fn winapi_binary<FT, W, WR, F, U>(
    initial_buffer: &mut dyn WriteBuffer,
    grow_strategy: &dyn GrowStrategy,
    api_wrapper: W,
    finalize: F,
) -> Result<U, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<*mut FT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    let growable_buffer = GrowableBuffer::<FT, *mut FT>::new(initial_buffer, grow_strategy);
    winapi_generic(growable_buffer, api_wrapper, finalize)
}

pub fn winapi_small_binary<FT, W, WR, F, U>(
    api_wrapper: W,
    finalize: F,
) -> Result<U, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<*mut FT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    let mut initial_buffer = StackBuffer::<1024>::new();
    let grow_strategy = GrowByNearestNibble::new();
    winapi_binary(&mut initial_buffer, &grow_strategy, api_wrapper, finalize)
}

pub fn winapi_large_binary<FT, W, WR, F, U>(
    api_wrapper: W,
    finalize: F,
) -> Result<U, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<*mut FT>) -> WR,
    F: FnMut(FrozenBuffer<FT>) -> Result<U, std::io::Error>,
{
    let mut initial_buffer = StackBuffer::<65536>::new();
    let grow_strategy = GrowByQuarterKibi::new();
    winapi_binary(&mut initial_buffer, &grow_strategy, api_wrapper, finalize)
}

pub fn winapi_path_buf<W, WR>(api_wrapper: W) -> Result<std::path::PathBuf, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<PWSTR>) -> WR,
{
    let mut initial_buffer = StackBuffer::<CAPACITY_FOR_PATHS>::new();
    let grow_strategy = GrowByDoubleNibbles::new(CAPACITY_FOR_PATHS.try_into().unwrap());
    let growable_buffer = GrowableBuffer::<u16, PWSTR>::new(&mut initial_buffer, &grow_strategy);
    winapi_generic(growable_buffer, api_wrapper, |frozen_buffer| {
        Ok(frozen_buffer.to_path_buf().unwrap_or_default())
    })
}

pub fn winapi_string<W, WR>(
    lossy_ok: bool,
    api_wrapper: W,
) -> Result<Result<String, OsString>, std::io::Error>
where
    WR: ToResult,
    W: FnMut(&mut Argument<PWSTR>) -> WR,
{
    let mut initial_buffer = StackBuffer::<CAPACITY_FOR_NAMES>::new();
    let grow_strategy = GrowByBumpToNibble::new();
    let growable_buffer = GrowableBuffer::<u16, PWSTR>::new(&mut initial_buffer, &grow_strategy);
    winapi_generic(growable_buffer, api_wrapper, |frozen_buffer| {
        Ok(frozen_buffer.to_string(lossy_ok))
    })
}
