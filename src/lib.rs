
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

pub type Result<T> = result::Result<T, Error>;

