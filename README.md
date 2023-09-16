# Introduction

Welcome to the grob crate!

grob is short for growable buffer.

Many Windows API functions require the caller to provide a buffer.  The pattern goes something like
this...
* Call the function with an initial buffer and size
* If that works then process the returned data
* If that does not work because the buffer is too small then create a larger buffer and try again
* If that does not work for any other reason then deal with the error

There are copious examples of a growable buffer including a version in the Rust Standard Library.
There is a lack of consistency amoungst all the examples.  Some versions continue trying
indefinately.  Some versions make an arbitrary number of attempts like three then give up.  Some
versions double the size of the new buffer.  Some versions increase the size by a fixed amount like
128 bytes.  Even Microsoft's API examples are inconsistent.

The goal with this crate is to provide a single high quality growable buffer that any Rust developer
can easily use.

## License

`grob` is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.

## Build Status

![Clippy](https://github.com/github/docs/actions/workflows/clippy.yml/badge.svg)
