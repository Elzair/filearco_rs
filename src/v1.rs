//! This module creates and manages a FileArco v1 archive file.
//!
//! # Example
//!
//! This example opens an example archive and outputs the text of all 3
//! stored files.
//!
//! ```rust
//! extern crate filearco;
//!
//! use std::path::Path;
//!
//! let archive_path = Path::new("testarchives/simple_v1.fac");
//! let archive = filearco::v1::FileArco::new(archive_path).ok().unwrap();
//! let cargo_toml = archive.get("Cargo.toml").unwrap();
//! println!("{}", cargo_toml.as_str().ok().unwrap());
//! let license_mit = archive.get("LICENSE-MIT").unwrap();
//! println!("{}", license_mit.as_str().ok().unwrap());
//! let license_apache = archive.get("LICENSE-APACHE").unwrap();
//! println!("{}", license_apache.as_str().ok().unwrap());
//! ```

use std::collections::HashMap;
use std::convert::AsRef;
use std::error;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::mem;
use std::slice;
use std::str;
use std::sync::Arc;
use std::path::Path;

use bincode::{serialize, deserialize, Bounded, Infinite};
use crc::crc64::checksum_iso as checksum;
use memmap::{Mmap, Protection};
use page_size::get as get_page_size;

use super::{Error, FILEARCO_ID, Result};
use file_data::FileData;

const VERSION_NUMBER: u64 = 1;

/// This represents an open, memory-mapped FileArco v1 archive file.
pub struct FileArco {
    inner: Arc<Inner>,
}

impl FileArco {
    /// This method tries to map a file specified by `path` into memory
    /// and process it as a FileArco V1 archive file.
    ///
    /// # Arguments
    ///
    /// * path - file path of archive file
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(path).ok().unwrap(); 
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let map = Mmap::open_path(path.as_ref(), Protection::Read)?;

        // Create test Header to determine size of encoded header.
        let test_header = Header::new(
            get_page_size() as u64,
            0,
            0,
            0
        );
        let test_header_encoded = serialize(&test_header, Infinite).unwrap();

        // `header_checksum` is bounded to the size of a u64 (probably 8 bytes).
        let checksum_size = mem::size_of::<u64>();

        // Make sure file is large enough to contain a FileArco v1 header.
        if map.len() < test_header_encoded.len() + checksum_size {
            return Err(Error::FileArcoV1(FileArcoV1Error::FileTooSmall));
        }

        // Read in header.
        let (header, checksum1): (Header, u64) = unsafe {
            let ptr = map.ptr().offset(0);
            let sl = slice::from_raw_parts(
                ptr,
                test_header_encoded.len()
            );

            (
                deserialize(sl).unwrap(),
                checksum(&sl)
            )
        };

        // Read in header checksum.
        let header_checksum: u64 = unsafe {
            let ptr = map.ptr().offset(test_header_encoded.len() as isize);
            let sl = slice::from_raw_parts(ptr, checksum_size);
            deserialize(sl).unwrap()
        };

        // Ensure header is valid.
        if header.id != *FILEARCO_ID {
            return Err(Error::FileArcoV1(FileArcoV1Error::NotArchive));
        }

        if header.version_number != 1 {
            return Err(Error::FileArcoV1(FileArcoV1Error::NotV1Archive));
        }

        if checksum1 != header_checksum {
            return Err(Error::FileArcoV1(FileArcoV1Error::CorruptedHeader));
        }

        if (map.len() as u64) < header.file_length {
            return Err(Error::FileArcoV1(FileArcoV1Error::FileTruncated));
        }

        // Read in entries data.
        let (entries, checksum2) = unsafe {
            let offset = checksum_size + test_header_encoded.len();
            let ptr = map.ptr().offset(offset as isize);
            let sl = slice::from_raw_parts(ptr, header.entries_length as usize);

            (
                deserialize(sl).unwrap(),
                checksum(&sl)
            )
        };

        // Ensure entries table is valid.
        if checksum2 != header.entries_checksum {
            return Err(Error::FileArcoV1(FileArcoV1Error::CorruptedEntriesTable));
        }

        Ok(FileArco {
            inner: Arc::new(Inner {
                file_offset: header.file_offset,
                page_size: header.page_size,
                entries: entries,
                map: map,
            })
        })
    }

    /// This method retrieves a file from the archive, if it exists.
    ///
    /// # Arguments
    ///
    /// * file_path - name of file to retrieve
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(path).ok().unwrap(); 
    /// 
    /// let cargo_toml = file_data.get("Cargo.toml").unwrap();
    /// ```
    pub fn get<P: AsRef<str>>(&self, file_path: P) -> Option<FileRef> {
        if let Some(entry) = self.inner.entries.files.get(file_path.as_ref()) {
            let offset = (self.inner.file_offset + entry.offset) as isize;
            let address = unsafe { self.inner.map.ptr().offset(offset) };

            Some(FileRef {
                address: address,
                length: entry.length,
                aligned_length: entry.aligned_length,
                checksum: entry.checksum,
                inner: self.inner.clone(),
            })
        }
        else {
            None
        }
    }

    /// This method returns the memory page size of the system used to create
    /// the archive file.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(path).ok().unwrap(); 
    /// println!("{}", file_data.page_size());
    /// ```
    pub fn page_size(&self) -> u64 {
        self.inner.page_size
    }
    
    /// This method creates a FileArco v1 archive file, populates it with
    /// the specified files, and writes the result to the standard output.
    ///
    /// # Arguments
    ///
    /// * file_data - file paths and other metadata of the input files
    ///
    /// * out_path - file path for archive file
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::fs::File;
    /// use std::io;
    /// use std::path::Path;
    ///
    /// let base_path = Path::new("testarchives/reqchandocs");
    /// let file_data = filearco::get_file_data(base_path).ok().unwrap();
    ///
    /// filearco::v1::FileArco::make(file_data, io::stdout()).ok().unwrap();
    /// ```
    pub fn make<H: Write>(file_data: FileData, mut out_file: H) -> Result<()> {
        let base_path = file_data.path();
   
        // Create entries table and serialize it.
        let entries = Entries::new(file_data);
        let entries_encoded: Vec<u8> = serialize(&entries, Infinite).unwrap();

        // Create header, serialize it, and write it to archive.
        let header = Header::new(get_page_size() as u64,
                                 entries_encoded.len() as u64,
                                 entries.total_aligned_length(),
                                 checksum(&entries_encoded));
        let header_encoded = serialize(&header, Infinite).unwrap();
        out_file.write_all(&header_encoded)?;

        // Compute header checksum, serialize it, and write it to archive.
        let header_checksum = checksum(&header_encoded);
        let header_checksum_encoded = serialize(
            &header_checksum,
            Bounded(mem::size_of::<u64>() as u64)
        ).unwrap();
        out_file.write_all(&header_checksum_encoded)?;
        
        // Write serialized entries table to archive.
        out_file.write_all(&entries_encoded)?;

        // Pad archive with zeros to ensure files begin at a multiple of `page_size`.
        let start_length = header_encoded.len() + header_checksum_encoded.len() +
            entries_encoded.len();
        let padding_length = (header.file_offset as usize) - start_length;
        let padding: Vec<u8> = vec![0u8; padding_length];
        out_file.write_all(&padding)?;

        // Began writing files to archive.
        for (path, entry) in &entries.files {
            let full_path = base_path.to_path_buf().join(Path::new(&path));

            // Read in input file contents and write it to archive.
            let mut in_file = File::open(full_path)?;
            let mut buffer = Vec::<u8>::with_capacity(entry.length as usize); 
            in_file.read_to_end(&mut buffer)?;
            out_file.write_all(&buffer)?;
            
            // Pad archive with zeros to ensure next file begins at a multiple of 4096.
            let padding_length = entry.aligned_length - entry.length;
            let padding: Vec<u8> = vec![0u8; padding_length as usize];
            out_file.write_all(&padding)?;
        }
        
        Ok(())
    }
}

/// This struct represents a reference to a slice of memory containing
/// a requested file from the archive.
#[allow(dead_code)]
pub struct FileRef {
    address: *const u8,
    length: u64,
    aligned_length: u64,
    checksum: u64,
    // Holding a reference to the memory mapped file ensures it will not be
    // unmapped until we finish using it.
    inner: Arc<Inner>,
}

impl FileRef {
    /// This method ensures the file contents have not been corrupted.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::mem;
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(path).unwrap(); 
    /// 
    /// let cargo_toml = file_data.get("Cargo.toml").unwrap();
    /// assert!(cargo_toml.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        let sl = self.as_slice();
        let checksum_computed = checksum(sl);

        self.checksum == checksum_computed
    }
 
    /// This method retrieves a byte array representing the contents of a `FileRef`.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::mem;
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(path).unwrap(); 
    /// 
    /// let cargo_toml = file_data.get("Cargo.toml").unwrap();
    /// let cargo_toml_slice = cargo_toml.as_slice();
    /// let cargo_toml_text = unsafe { mem::transmute::<&[u8], &str>(cargo_toml_slice) };
    /// println!("{}", cargo_toml_text);
    /// ```
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.address, self.length as usize)
        }
    }
 
    /// This method retrieves a string representing the contents of a `FileRef`.
    /// It returns an error if the file contents do not represent a valid
    /// UTF-8 string.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(&path).unwrap(); 
    /// 
    /// let license = file_data.get("LICENSE-APACHE").unwrap();
    /// let license_text = license.as_str().ok().unwrap();
    /// println!("{}", license_text);
    /// ```
    pub fn as_str(&self) -> Result<&str> {
        let sl = unsafe {
            slice::from_raw_parts(self.address, self.length as usize)
        };

        let s = str::from_utf8(sl)?;

        Ok(s)
    }

    /// This method returns a tuple with a raw pointer to the beginning
    /// of the file and the page-aligned length of the file.
    ///
    /// # Unsafety
    ///
    /// Callers should not use this method to modify the contents
    /// of the file. Undefined Behavior will result.
    ///
    /// # Example
    ///
    /// This example uses the `memadvise` crate to advise the operating system
    /// to load the entire file into physical RAM.
    ///
    /// ```rust
    /// extern crate filearco;
    /// extern crate memadvise;
    ///
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(&path).unwrap(); 
    /// 
    /// let license = file_data.get("LICENSE-APACHE").unwrap();
    /// let (ptr, len) = license.as_raw();
    /// 
    /// memadvise::advise(ptr, len, memadvise::Advice::WillNeed).ok().unwrap();
    /// ```
    pub fn as_raw(&self) -> (*mut (), usize) {
        (self.address as *mut (), self.aligned_length as usize)
    }

    /// This method retrieves the length of the file.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate filearco;
    ///
    /// use std::mem;
    /// use std::path::Path;
    ///
    /// let path = Path::new("testarchives/simple_v1.fac");
    /// let file_data = filearco::v1::FileArco::new(path).unwrap(); 
    /// 
    /// let cargo_toml = file_data.get("Cargo.toml").unwrap();
    /// println!("File length: {}", cargo_toml.len());
    /// ```
    pub fn len(&self) -> u64 {
        self.length
    }
}

/// Error container for handling FileArco v1 archives
#[derive(Debug)]
pub enum FileArcoV1Error {
    /// Entry table's computed checksum did not match the one stored in the file.
    CorruptedEntriesTable,
    /// Header's computed checksum did not match the one stored in the file.
    CorruptedHeader,
    /// File is too small for the header of a FileArco v1 archive.
    FileTooSmall,
    /// File is a valid FileArco v1 archive but it has been truncated.
    FileTruncated,
    /// File does not have a valid identifier.
    NotArchive,
    /// File has a valid identifier but an incorrect version number.
    NotV1Archive,
    /// Something weird happened.
    Other,
}

impl fmt::Display for FileArcoV1Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FileArcoV1Error::CorruptedEntriesTable => {
                write!(fmt, "Corrupted entries table")
            },
            FileArcoV1Error::CorruptedHeader => {
                write!(fmt, "Corrupted header")
            },
            FileArcoV1Error::FileTooSmall => {
                write!(fmt, "File either too small for FileArco v1 archive or truncated")
            },
            FileArcoV1Error::FileTruncated => {
                write!(fmt, "File truncated")
            },
            FileArcoV1Error::NotArchive => {
                write!(fmt, "Not FileArco archive")
            },
            FileArcoV1Error::NotV1Archive => {
                write!(fmt, "Not FileArco v1 archive")
            },
            FileArcoV1Error::Other => {
                write!(fmt, "Something weird happened")
            },
        }
    }
}

impl error::Error for FileArcoV1Error {
    fn description(&self) -> &str {
        static CORRUPTED_ENTRIES_TABLE: &'static str = "Corrupted entries table";
        static CORRUPTED_HEADER: &'static str = "Corrupted header";
        static FILE_TOO_SMALL: &'static str = "File either too small for FileArco v1 archive or truncated";
        static FILE_TRUNCATED: &'static str = "File truncated";
        static NOT_ARCHIVE: &'static str = "Not FileArco archive";
        static NOT_V1_ARCHIVE: &'static str = "Not FileArco v1 archive";
        static OTHER: &'static str = "Something weird happened";

        match *self {
            FileArcoV1Error::CorruptedEntriesTable => {
                CORRUPTED_ENTRIES_TABLE
            },
            FileArcoV1Error::CorruptedHeader => {
                CORRUPTED_HEADER
            },
            FileArcoV1Error::FileTooSmall => {
                FILE_TOO_SMALL
            },
            FileArcoV1Error::FileTruncated => {
                FILE_TRUNCATED
            },
            FileArcoV1Error::NotArchive => {
                NOT_ARCHIVE
            },
            FileArcoV1Error::NotV1Archive => {
                NOT_V1_ARCHIVE
            },
            FileArcoV1Error::Other => {
                OTHER
            }
        }
    }

    fn cause(&self) -> Option<&error::Error> { None }
}

struct Inner {
    file_offset: u64,
    page_size: u64,
    entries: Entries,
    map: Mmap,
}

#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Header {
    id: [u8; 8],
    version_number: u64,
    file_length: u64,
    file_offset: u64,
    page_size: u64,
    entries_length: u64,
    entries_checksum: u64,
}

impl Header {
    fn new(page_size: u64,
           entries_length: u64,
           file_contents_length: u64,
           entries_checksum: u64) -> Self {
        // Serialize test struct to determine `file_offset`.
        let test_header = Header {
            id: *FILEARCO_ID,
            version_number: VERSION_NUMBER,
            file_length: 0,
            file_offset: 0,
            page_size: page_size,
            entries_length: entries_length,
            entries_checksum: entries_checksum,
        };
        let test_header_encoded = serialize(&test_header, Infinite).unwrap();
        let header_length = test_header_encoded.len() as u64;

        let file_offset = get_aligned_length(header_length + entries_length);
        let file_length = file_offset + file_contents_length;

        Header {
            id: *FILEARCO_ID,
            version_number: VERSION_NUMBER,
            file_length: file_length,
            file_offset: file_offset,
            page_size: page_size,
            entries_length: entries_length,
            entries_checksum: entries_checksum,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Entries {
    files: HashMap<String, Entry>,
}

impl Entries {
    fn new(file_data: FileData) -> Self {
        let mut files = HashMap::new();
        
        for datum in file_data.into_vec() {
            let aligned_length = get_aligned_length(datum.len());

            files.insert(datum.name(),
                         Entry {
                             offset: 0,
                             length: datum.len(),
                             aligned_length: aligned_length,
                             checksum: datum.checksum(),
                         }
            );
        }

        let mut offset = 0;
        let keys = files.keys().cloned().collect::<Vec<_>>();

        for key in keys {
            let val = files.get_mut(&key).unwrap();
            val.offset = offset;
            offset = offset + val.aligned_length;
        }

        Entries {
            files: files 
        }
    }

    fn total_aligned_length(&self) -> u64 {
        let mut total_length = 0_u64;
        
        let keys = self.files.keys().cloned().collect::<Vec<_>>();

        for key in keys {
            let val = self.files.get(&key).unwrap();
            total_length = total_length + val.aligned_length;
        }

        total_length
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Entry {
    offset: u64,
    length: u64,
    aligned_length: u64,
    checksum: u64,
}

/// This function returns the smallest multiple of 2^12 (i.e. 4096)
/// greater than or equal to the given length.
///
/// # Arguments
///
/// * length - the input number
#[inline]
fn get_aligned_length(length: u64) -> u64 {
    let page_size = get_page_size() as u64;

    // Assume memory page size is a power of 2.
    (length + (page_size-1)) & !(page_size-1)
}

#[cfg(test)]
mod tests {
    use std::fs::create_dir_all;

    use memadvise::{advise, Advice};
    
    use super::super::file_data::FileDatum;
    use super::*;

    fn get_file_data_stub<P: AsRef<Path>>(base_path: P) -> Result<FileData> {
        let mut data = Vec::<FileDatum>::new();
        data.push(FileDatum::new(
            String::from("Cargo.toml"),
            328,
            10574576474013701409,
        ));
        data.push(FileDatum::new(
            String::from("LICENSE-APACHE"),
            10771,
            8740797956101379381,
        ));
        data.push(FileDatum::new(
            String::from("LICENSE-MIT"),
            1082,
            13423357612537305206,
        ));
        
        Ok(FileData::new(
            base_path.as_ref().to_path_buf(),
            data,
        ))
    }

    fn get_simple() -> Vec<String> {
        let mut v = Vec::<String>::new();

        v.push(String::from("Cargo.toml"));
        v.push(String::from("LICENSE-APACHE"));
        v.push(String::from("LICENSE-MIT"));

        v
    }

    #[test]
    fn test_v1_get_rounded_length() {
        assert_eq!(get_aligned_length(0), 0);
        assert_eq!(get_aligned_length(4096), 4096);
        assert_eq!(get_aligned_length(4096+1), 2 * 4096);
        assert_eq!(get_aligned_length(2*4096 - 1), 2 * 4096);
    }

    #[test]
    fn test_v1_entries_new() {
        let file_data = get_file_data_stub(&Path::new("testarchives/simple")).ok().unwrap();
        let entries = Entries::new(file_data);

        let simple = get_simple();

        for name in simple.iter() {
            assert!(entries.files.contains_key(name));
        }
    }

    #[test]
    fn test_v1_filearco_make() {
        let base_path = Path::new("testarchives/simple");
        let file_data = get_file_data_stub(base_path).ok().unwrap();

        let archive_path = Path::new("tmptest/test_v1_filearco_make.fac");

        // Create directory if it does not exist
        if let Some(parent) = archive_path.parent() {
            create_dir_all(parent).ok().unwrap();
        }

        let archive_file = File::create(archive_path).ok().unwrap();
        FileArco::make(file_data, archive_file).ok().unwrap();
    }

    #[test]
    fn test_v1_filearco_new() {
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let simple = get_simple();

        match FileArco::new(archive_path) {
            Ok(archive) => {
                for name in simple.iter() {
                    assert!(archive.inner.entries.files.contains_key(name));
                }
            },
            Err(err) => {
                println!("test_v1_filearco_new {}", err.to_string());
                assert!(false); },
        }
    }

    #[test]
    fn test_v1_filearco_page_size() {
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let archive = FileArco::new(archive_path).ok().unwrap();

        assert_eq!(archive.page_size(), 4096);
    }

    #[test]
    fn test_v1_filearco_get() {
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let archive = FileArco::new(archive_path).ok().unwrap();

        let base_path = Path::new("testarchives/simple");
        let simple = get_file_data_stub(base_path).ok().unwrap();
        let svec = simple.into_vec();

        for entry in svec.iter() {
            if let Some(fileref) = archive.get(entry.name()) {
                assert_eq!(fileref.len(), entry.len());
                assert!(fileref.is_valid());
            }
            else {
                assert!(false);
            }
        }
    }

    #[test]
    fn test_v1_fileref_as_slice() {
        let dir_path = Path::new("testarchives/simple");
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let archive = FileArco::new(archive_path).ok().unwrap();

        let simple = get_file_data_stub(dir_path).ok().unwrap();
        let base_path = simple.path();
        let svec = simple.into_vec();

        for entry in svec.into_iter() {
            let full_name = format!(
                "{}/{}",
                &base_path.to_string_lossy(),
                &entry.name()
            );
            let full_path = Path::new(&full_name);

            // Read in input file contents.
            let mut in_file = File::open(full_path).ok().unwrap();
            let mut contents = Vec::<u8>::with_capacity(entry.len() as usize); 
            in_file.read_to_end(&mut contents).ok().unwrap();
            
            let archived_file = archive.get(&entry.name()).unwrap();
            let length2 = archived_file.len();

            assert_eq!(entry.len(), archived_file.as_slice().len() as u64);
            assert_eq!(length2, archived_file.as_slice().len() as u64);
            assert_eq!(contents, archived_file.as_slice());
        }
    }
    
    #[test]
    fn test_v1_fileref_as_raw() {
        let dir_path = Path::new("testarchives/simple");
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let archive = FileArco::new(archive_path).ok().unwrap();

        let simple = get_file_data_stub(dir_path).ok().unwrap();
        let svec = simple.into_vec();

        for entry in svec.into_iter() {
            let archived_file = archive.get(&entry.name()).unwrap();

            let (ptr, len) = archived_file.as_raw();
            advise(ptr, len, Advice::WillNeed).ok().unwrap();
            advise(ptr, len, Advice::DontNeed).ok().unwrap();
        }
    }
}
