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

/// What action to take after an operating system call: Commit, Grow, or NoData
///
#[derive(Debug)]
pub enum FillBufferAction {
    /// The operating system call was successful and there is usable data in the buffer.  Normally,
    /// [`freeze`][f] is called to turn the buffer into a [`FrozenBuffer`][fb] so the data can be
    /// accessed.
    ///
    /// [f]: crate::GrowableBuffer::freeze
    /// [fb]: crate::FrozenBuffer
    Commit,
    /// Grow the buffer using the [`GrowStrategy`][gs].  Typically, the operating system call is
    /// tried again with the larger buffer.
    ///
    /// [gs]: crate::GrowStrategy
    Grow,
    /// The operating system call was successful but there is no data available.  Despite the lack
    /// of data, [`freeze`][f] is usually called to turn the buffer into a [`FrozenBuffer`][fb] so
    /// there is one code to handle the have-data and the no-data possibilities.
    ///
    /// [f]: crate::GrowableBuffer::freeze
    /// [fb]: crate::FrozenBuffer
    NoData,
}

/// The result of an operating system call.
///
/// On success, the [`FillBufferAction`] indicates what should happen next.  There are three
/// choices:
///
/// - Try again with a larger buffer ([`Grow`][g])
/// - Process the data ([`Commit`][c])
/// - Handle a successful call that provided no data ([`NoData`][n])
///
/// Success means that either the operating system call worked and optionally provided data or
/// returned an error indicating the buffer size is too small.
///
/// On error, the value is a [`std::io::Error`] that was returned from the operating system call.
///
/// [g]: crate::FillBufferAction::Grow
/// [c]: crate::FillBufferAction::Commit
/// [n]: crate::FillBufferAction::NoData
///
pub type FillBufferResult = Result<FillBufferAction, std::io::Error>;
