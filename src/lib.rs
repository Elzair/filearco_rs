
extern crate bincode;
extern crate crc;
extern crate memmap;
extern crate memadvise;
extern crate page_size;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate walkdir;

const FILEARCO_MAGIC_NUMBER: u64 = 0xF11EA4C0F11EA4C0; // It kinda looks like FILEARC0FILEARC0

mod file_data;
pub mod v1;

pub use file_data::{get as get_file_data, FileData};
