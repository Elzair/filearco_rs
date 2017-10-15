[Documentation](https://docs.rs/filearco)

[![Linux Status](https://travis-ci.org/Elzair/filearco_rs.svg?branch=master)](https://travis-ci.org/Elzair/filearcho_rs)
[![Build status](https://ci.appveyor.com/api/projects/status/gkeu80ru3gq3b7sg?svg=true)](https://ci.appveyor.com/project/Elzair/filearco-rs)

# Introduction

`filearco_rs` is a Rust crate for creating and reading a simple archive format. It was designed for game development, but can be used for other purposes.

It also includes a command-line utility, `filearco`, that writes all the files in a directory hierarchy into an archive.

# Example

This example demonstrates creating a FileArco version 1 archive file and retrieving a text file from that same archive.

```rust
extern crate filearco;

use std::path::Path

fn main() {
    let path = Path::new("archive.fac");
    let archive = filearco::v1::FileArco::new(path).ok().unwrap();
    
    let license = archive.get("LICENSE-MIT").unwrap()
    let license_text = license.as_str().ok().unwrap();
    println!("{}", license_text);
}

```

# File Format

## Version 1

**NOTE:** All data is stored in LSB (i.e. "little endian") byte order.

```rust
// Ofset 0x00: Start of file
#[repr(C)]
struct Header {
    id: [u8; 8],           // b"FILEARCO"
    version_number: u64    // 1
    file_offset: u64,      // Offset to first file
    page_size: u64,        // Memory Page Size of system that created file
    entries_length: u64,   // Length of Entries table (in bytes)
    entries_checksum: u64, // CRC64-ISO checksum of Entries table
}

// Offset 0x30:
header_checksum: u64 // CRC64-ISO checksum of Header

// Offset 0x38:
// Start of serialized HashMap<String, Entry>
number_of_entries: u64

// Offset 0x40: Start of first file's metadata
file_name_length: u64,             // Length of file path (in bytes)
file_name: [u8; file_name_length]  // File path as raw UTF-8 string

#[repr(C)
struct Entry {
    offset: u64, 
    length: u64,
    aligned_length: u64,
    checksum: u64
}
// Metadata for the second file (and so on) follow directly after

// NOTE: the last Entry is followed by enough zeros to make the next section
// start at a multiple of header.page_size

// Offset M * header.page_size: Start of file contents section

// Offset header.file_offset + entry.offset: Start of a file's contents
contents: [u8; entry.length] // Contents of file as byte array

// NOTE: Each contents array is followed by enough zeros to make the next file
// contents array start at a multiple of header.page_size

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
