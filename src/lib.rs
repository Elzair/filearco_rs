//! This crate creates and reads FileArco archives.
//!
//! # Example
//!
//! This example creates a FileArco v1 archive file containing 3 text files.
//! It then opens the newly created file and outputs the text of all 3
//! stored files.
//!
//! ```rust
//! extern crate filearco;
//!
//! use std::path::Path;
//!
//! let base_path = Path::new("testarchives/simple");
//! let archive_path = Path::new("tmptest/doctest_simple_v1.fac");
//! let file_data = filearco::get_file_data(base_path).ok().unwrap();
//! filearco::v1::FileArco::make(file_data, archive_path).ok().unwrap();
//! let archive = filearco::v1::FileArco::new(archive_path).ok().unwrap();
//! let cargo_toml = archive.get("Cargo.toml").unwrap();
//! println!("{}", cargo_toml.as_str());
//! let license_mit = archive.get("LICENSE-MIT").unwrap();
//! println!("{}", license_mit.as_str());
//! let license_apache = archive.get("LICENSE-APACHE").unwrap();
//! println!("{}", license_apache.as_str());
//! ```

extern crate bincode;
extern crate crc;
extern crate memmap;
extern crate page_size;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate walkdir;

const FILEARCO_MAGIC_NUMBER: u64 = 0xF11EA4C0F11EA4C0; // It kinda looks like FILEARC0FILEARC0

mod file_data;
pub mod v1;

pub use file_data::{get as get_file_data, FileData, FileDataError};

use std::io;
use std::result;
use std::str;

/// This is the top level Error for this crate.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Utf8(str::Utf8Error),
    Walkdir(walkdir::Error),
    FileArcoV1(v1::FileArcoV1Error),
    FileData(FileDataError),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(err: str::Utf8Error) -> Error {
        Error::Utf8(err)
    }
}

impl From<walkdir::Error> for Error {
    fn from(err: walkdir::Error) -> Error {
        Error::Walkdir(err)
    }
}

impl From<v1::FileArcoV1Error> for Error {
    fn from(err: v1::FileArcoV1Error) -> Error {
        Error::FileArcoV1(err)
    }
}

/// This is the result type.
pub type Result<T> = result::Result<T, Error>;

