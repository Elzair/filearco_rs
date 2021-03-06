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
//! use std::fs::File;
//! use std::path::Path;
//!
//! // Retrieve metadata on files to archive.
//! let base_path = Path::new("testarchives/simple");
//! let file_data = filearco::get_file_data(base_path).ok().unwrap();
//!
//! // Create FileArco v1 archive file.
//! // Make sure parent directory exists first.
//! let archive_path = Path::new("tmptest/doctest_simple_v1.fac");
//! let archive_file = File::create(archive_path).ok().unwrap();
//! filearco::v1::FileArco::make(file_data, archive_file).ok().unwrap();
//!
//! // Map archive file into memory.
//! let archive = filearco::v1::FileArco::new(archive_path).ok().unwrap();
//!
//! // Retrieve and print contents of archive.
//! let cargo_toml = archive.get("Cargo.toml").unwrap();
//! println!("{}", cargo_toml.as_str().ok().unwrap());
//! let license_mit = archive.get("LICENSE-MIT").unwrap();
//! println!("{}", license_mit.as_str().ok().unwrap());
//! let license_apache = archive.get("LICENSE-APACHE").unwrap();
//! println!("{}", license_apache.as_str().ok().unwrap());
//! ```

extern crate bincode;
extern crate crc;
extern crate memmap;
extern crate page_size;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate walkdir;

#[cfg(test)]
extern crate memadvise;

const FILEARCO_ID: &'static [u8; 8] = b"FILEARCO";

mod file_data;
pub mod v1;

pub use file_data::{get as get_file_data, FileData, FileDataError};

use std::error;
use std::fmt;
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

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Io(ref err) => err.fmt(fmt),
            &Error::Utf8(ref err) => err.fmt(fmt),
            &Error::Walkdir(ref err) => err.fmt(fmt),
            &Error::FileArcoV1(ref err) => err.fmt(fmt),
            &Error::FileData(ref err) => err.fmt(fmt),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Io(ref err) => err.description(),
            &Error::Utf8(ref err) => err.description(),
            &Error::Walkdir(ref err) => err.description(),
            &Error::FileArcoV1(ref err) => err.description(),
            &Error::FileData(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match self {
            &Error::Io(ref err) => err.cause(),
            &Error::Utf8(ref err) => err.cause(),
            &Error::Walkdir(ref err) => err.cause(),
            &Error::FileArcoV1(ref err) => err.cause(),
            &Error::FileData(ref err) => err.cause(),
        }
    }
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

