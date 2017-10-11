[Documentation](https://docs.rs/filearco)

[![Linux Status](https://travis-ci.org/Elzair/filearco_rs.svg?branch=master)](https://travis-ci.org/Elzair/filearcho_rs)
[![Build status](https://ci.appveyor.com/api/projects/status/yf2d627xup9gnx4e?svg=true)](https://ci.appveyor.com/project/Elzair/filearco)


# Introduction

`filearco_rs` is a Rust crate for creating and reading a simple archive format. It was designed for game development, but can be used for other purposes.

# Example

This example demonstrates creating a FileArco version 1 archive file and retrieving a text file from that same archive.

```rust
extern crate filearco;

use std::path::Path

fn main() {
    let path = Path::new("archive.fac");
    let archive = filearco::v1::FileArco::new(&path).unwrap();
    
    let license = archive.get("LICENSE-MIT").unwrap()
    let license_text = license.as_str().ok().unwrap();
    println!("{}", license_text);
}

```

# Platforms

`filearco_rs` should Work on Windows and any POSIX compatible system (Linux, Mac OSX, etc.).

`filearco_rs` is continuously tested on:
  * `x86_64-unknown-linux-gnu` (Linux)
  * `i686-unknown-linux-gnu`
  * `x86_64-unknown-linux-musl` (Linux w/ [MUSL](https://www.musl-libc.org/))
  * `i686-unknown-linux-musl`
  * `x86_64-apple-darwin` (Mac OSX)
  * `i686-apple-darwin`
  * `x86_64-pc-windows-msvc` (Windows)
  * `i686-pc-windows-msvc`
  * `x86_64-pc-windows-gnu`
  * `i686-pc-windows-gnu`

`filearco_rs` is continuously cross-compiled for:
  * `arm-unknown-linux-gnueabihf`
  * `aarch64-unknown-linux-gnu`
  * `mips-unknown-linux-gnu`
  * `aarch64-unknown-linux-musl`
  * `i686-linux-android`
  * `x86_64-linux-android`
  * `arm-linux-androideabi`
  * `aarch64-linux-android`
  * `i386-apple-ios`
  * `x86_64-apple-ios`
  * `i686-unknown-freebsd`
  * `x86_64-unknown-freebsd`
  * `x86_64-unknown-netbsd`
  * `asmjs-unknown-emscripten`
