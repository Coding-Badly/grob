# Changelog

## grob 0.1.2 (2024-02-11)
[v0.1.1...v0.1.2](https://github.com/Coding-Badly/grob/compare/v0.1.1...v0.1.2)

### Added

- Two Miri tests to cover a zero-sized StackBuffer

### Changed

- Simplify and improve the grow strategies
- Various improvements and simplifications to the examples
- Remove some incorrect asserts
- Skip Miri tests on the examples; all of the API calls are not emulated

### Fixed

- Fix a buffer alignment bug
- Eliminate the use of a null pointer that triggered a Miri error
- Resolve all clippy issues

### Documentation

- Add complete documentation
- Make some internal links more consistent

## grob 0.1.1 (2023-05-29)
[v0.1.0...v0.1.1](https://github.com/Coding-Badly/grob/compare/v0.1.0...v0.1.1)

### Added

- Build docs for x86_64-pc-windows-msvc as that's the target platform

## grob 0.1.0 (2023-05-26)

- Initial release: https://github.com/Coding-Badly/grob
