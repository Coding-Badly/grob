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

use std::marker::PhantomData;

use crate::buffer::os::ALIGNMENT;
use crate::traits::GrowStrategy;
use crate::win::SIZE_OF_WCHAR;

/// Adjustments made by [GrowToNearestNibbleWithExtra] when calculating the next buffer capacity
///
/// [EXTRA][1] is either zero or SIZE_OF_WCHAR.  It's SIZE_OF_WCHAR to guarantee space for a `NULL`
/// terminator.  Internally, Microsoft has struggled with accommodating `NULL`s and determining
/// buffer capacities.  Including space for one extra element protects us from those mistakes.
///
/// [SCALE][2] is either one or two.  Some Windows API calls return the amount stored instead of the
/// amount needed.  Our only option is to guess what capacity the buffer should be.  The strategy is
/// to double the buffer capacity after each attempt.
///
/// [FLOOR][3] is an optional minimum value.  If not zero, the buffer capacity is never below this
/// value.  A non-zero [FLOOR][3] is appropriate for Windows API calls that have what is essentially
/// a recommended buffer capacity (e.g. `MAX_PATH * SIZE_OF_WCHAR`).
///
/// [1]: NearestNibbleAdjustments::EXTRA
/// [2]: NearestNibbleAdjustments::SCALE
/// [3]: NearestNibbleAdjustments::FLOOR
///
trait NearestNibbleAdjustments {
    const EXTRA: u64;
    const SCALE: u64;
    const FLOOR: u64;
}

/// This is the core implementation for all things that need a smallish static buffer
///
/// [GrowToNearestNibbleWithExtra] is combined with a [NearestNibbleAdjustments] to form a
/// [GrowStrategy] for a given use-case.  A combination is exposed to the world as a use-case
/// (e.g. [GrowForStaticText]).
///
struct GrowToNearestNibbleWithExtra<A: NearestNibbleAdjustments> {
    phantom: PhantomData<A>,
}

impl<A: NearestNibbleAdjustments> GrowToNearestNibbleWithExtra<A> {
    fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<A: NearestNibbleAdjustments> GrowStrategy for GrowToNearestNibbleWithExtra<A> {
    fn next_capacity(&self, _tries: usize, desired_capacity: u32) -> u32 {
        // With desired_capacity a u32, doing the math with u64 prevents all overlow possibilities.
        // Eliminate repeated casts
        let desired_capacity = desired_capacity as u64;
        // Determine the ceiling of the current number of nibbles.  Supports bumping to include
        // space for a NULL terminator (just in case of an API bug).
        let bumped_nibbles = (desired_capacity + A::EXTRA + 15) / 16;
        // Convert that to bytes optionally scaling
        let scaled_bytes = bumped_nibbles * 16 * A::SCALE;
        // Use the largest of the doubled value, desired_capacity, or the preconfigured floor.
        // Limit that to u32::MAX.
        scaled_bytes
            .max(desired_capacity)
            .max(A::FLOOR)
            .min(u32::MAX as u64) as u32
    }
}

/// A [NearestNibbleAdjustments] that just rounds the `desired_capacity` up to the next higher value
/// evenly divisible by 16.
///
struct NoAdjustments {}

impl NearestNibbleAdjustments for NoAdjustments {
    const EXTRA: u64 = 0;
    const SCALE: u64 = 1;
    const FLOOR: u64 = 0;
}

/// [`GrowStrategy`] appropriate for small binary data that is unlikely to change where the call
/// returns the buffer size needed.
///
/// This [`GrowStrategy`] works best when the operating system indicates the buffer size needed
/// (`desired_capacity` is known), that size is unlikely to change, and the buffer size is
/// relatively small.
///
/// This [`GrowStrategy`] rounds the buffer size to the next higher value that's evenly divisible by
/// 16.
///
/// The goals are:
///
///   * Be heap friendly by avoiding many small odd sized heap allocations
///   * For the API call to be successful after at most two attempts
///
/// [`GetLogicalProcessorInformationEx`][1] is a good example for this [`GrowStrategy`].
///
/// Favor the [`GrowForSmallBinary`] alias over using this strategy directly so your code can
/// naturally take advantage of improvements.
///
/// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/SystemInformation/fn.GetLogicalProcessorInformationEx.html
///
pub struct GrowToNearestNibble {
    inner: GrowToNearestNibbleWithExtra<NoAdjustments>,
}

impl GrowToNearestNibble {
    pub fn new() -> Self {
        Self {
            inner: GrowToNearestNibbleWithExtra::new(),
        }
    }
}

impl Default for GrowToNearestNibble {
    fn default() -> Self {
        Self::new()
    }
}

impl GrowStrategy for GrowToNearestNibble {
    fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32 {
        self.inner.next_capacity(tries, desired_capacity)
    }
}

/// Alias for the [`GrowToNearestNibble`] [`GrowStrategy`].
///
/// The [`GrowForSmallBinary`] alias should be favored over using [`GrowToNearestNibble`] directly.
/// Future versions may change the strategy for small static binary data.  By using this alias your
/// code will naturally take advantage of improvements.
///
pub type GrowForSmallBinary = GrowToNearestNibble;

/// A [NearestNibbleAdjustments] that rounds the `desired_capacity` up to the next higher value
/// evenly divisible by 16 after adding space for a `NULL` terminator.
///
struct AdjustForNull {}

impl NearestNibbleAdjustments for AdjustForNull {
    const EXTRA: u64 = SIZE_OF_WCHAR as u64;
    const SCALE: u64 = 1;
    const FLOOR: u64 = 0;
}

/// [`GrowStrategy`] appropriate for Windows API calls that return the number of characters that
/// need to be stored for success (the needed buffer size is returned).
///
/// This [`GrowStrategy`] works best when the operating system indicates the buffer size needed
/// (`desired_capacity` is known), that size is unlikely to change, and the buffer size is
/// relatively small.
///
/// This [`GrowStrategy`] rounds the buffer size to the next higher value that's evenly divisible by
/// 16 after adding space for a `NULL` terminator.
///
/// The goals are:
///
///   * Be heap friendly by avoiding many small odd sized heap allocations
///   * Avoid any operating system bugs involving the buffer size requested being incorrect because
///     the `NULL` is not considered
///   * For the API call to be successful after at most two attempts
///
/// [`GetUserNameW`][1] is a good example for this [`GrowStrategy`].
///
/// Favor the [`GrowForStaticText`] alias over using this strategy directly so your code can
/// naturally take advantage of improvements.
///
/// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/WindowsProgramming/fn.GetUserNameW.html
///
pub struct GrowToNearestNibbleWithNull {
    inner: GrowToNearestNibbleWithExtra<AdjustForNull>,
}

impl GrowToNearestNibbleWithNull {
    pub fn new() -> Self {
        Self {
            inner: GrowToNearestNibbleWithExtra::new(),
        }
    }
}

impl Default for GrowToNearestNibbleWithNull {
    fn default() -> Self {
        Self::new()
    }
}

impl GrowStrategy for GrowToNearestNibbleWithNull {
    fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32 {
        self.inner.next_capacity(tries, desired_capacity)
    }
}

/// Alias for the [`GrowToNearestNibbleWithNull`] [`GrowStrategy`].
///
/// The [`GrowForStaticText`] alias should be favored over using [`GrowToNearestNibbleWithNull`]
/// directly.  Future versions may change the strategy for static text data.  By using this alias
/// your code will naturally take advantage of improvements.
///
pub type GrowForStaticText = GrowToNearestNibbleWithNull;

/// A [NearestNibbleAdjustments] that rounds the `current_size` up to the next higher value evenly
/// divisible by 16 after adding space for a `NULL` terminator.  The target is that value doubled.
///
struct DoublePlusNull<const FLOOR: u64> {}

impl<const FLOOR: u64> NearestNibbleAdjustments for DoublePlusNull<FLOOR> {
    const EXTRA: u64 = SIZE_OF_WCHAR as u64;
    const SCALE: u64 = 2;
    const FLOOR: u64 = FLOOR;
}

/// [`GrowStrategy`] appropriate for Windows API calls that return the number of characters stored
/// (the needed buffer space is not available).
///
/// This [`GrowStrategy`] works best when the operating system does not provide the buffer size
/// needed, that size is unlikely to change, and the buffer size is relatively small.
///
/// This [`GrowStrategy`] rounds the buffer size to the next higher value that's evenly divisible by
/// 16 after adding space for a `NULL` terminator then doubles the value.
///
/// The goals are:
///
///   * Be heap friendly by avoiding many small odd sized heap allocations
///   * Avoid any operating system bugs involving the buffer size requested being incorrect because
///     the `NULL` is not considered
///   * Ensure the buffer grows quickly to avoid too many API calls
///
/// [`GetModuleFileNameW`][1] is a good example for this [`GrowStrategy`].
///
/// Favor the [`GrowForStoredIsReturned`] alias over using this strategy directly so your code can
/// naturally take advantage of improvements.
///
/// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/LibraryLoader/fn.GetModuleFileNameW.html
///
pub struct GrowByDoubleWithNull<const FLOOR: u64> {
    inner: GrowToNearestNibbleWithExtra<DoublePlusNull<FLOOR>>,
}

impl<const FLOOR: u64> GrowByDoubleWithNull<FLOOR> {
    pub fn new() -> Self {
        Self {
            inner: GrowToNearestNibbleWithExtra::new(),
        }
    }
}

impl<const FLOOR: u64> Default for GrowByDoubleWithNull<FLOOR> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const FLOOR: u64> GrowStrategy for GrowByDoubleWithNull<FLOOR> {
    fn next_capacity(&self, tries: usize, desired_capacity: u32) -> u32 {
        self.inner.next_capacity(tries, desired_capacity)
    }
}

/// Alias for the [`GrowByDoubleWithNull`] [`GrowStrategy`].
///
/// The [`GrowForStoredIsReturned`] alias should be favored over using [`GrowByDoubleWithNull`]
/// directly.  Future versions may change the strategy for operating system calls that return the
/// number of elements stored.  By using this alias your code will naturally take advantage of
/// improvements.
///
pub type GrowForStoredIsReturned<const FLOOR: u64> = GrowByDoubleWithNull<FLOOR>;

/// [`GrowStrategy`] appropriate for large binary data that may change between calls where the call
/// returns the buffer size needed.
///
/// This [`GrowStrategy`] works best when the operating system indicates the buffer size needed
/// (`desired_capacity` is known), that size may change between calls, and the buffer size is
/// relatively large.
///
/// This [`GrowStrategy`] rounds the buffer size to the next higher value that's evenly divisible by
/// 256 after adding space for alignment.
///
/// The goals are:
///
///   * Be heap friendly by avoiding small odd sized heap allocations
///   * For the API call to be successful after at most two attempts
///   * Ensure there is enough space to provide an aligned buffer
///
/// [`GetAdaptersAddresses`][1] is a good example for this [`GrowStrategy`].
///
/// [1]: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/NetworkManagement/IpHelper/fn.GetAdaptersAddresses.html
///
pub struct GrowToNearestQuarterKibi {}

impl GrowToNearestQuarterKibi {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for GrowToNearestQuarterKibi {
    fn default() -> Self {
        Self::new()
    }
}

impl GrowStrategy for GrowToNearestQuarterKibi {
    fn next_capacity(&self, _tries: usize, desired_capacity: u32) -> u32 {
        // With desired_capacity a u32, doing the math with u64 prevents all overlow possibilities.
        // Determine the ceiling of the current number of quarter kibis plus some for alignment.
        let quarter_kibis = (desired_capacity as u64 + 255 + ALIGNMENT as u64) / 256;
        // Convert to bytes
        let bytes = quarter_kibis * 256;
        // Limit the target to a value that fits in a u32.
        bytes.min(u32::MAX as u64) as u32
    }
}
