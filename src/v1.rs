use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::mem::{transmute, transmute_copy};
use std::slice;
use std::sync::Arc;
use std::path::Path;

use bincode::{serialize, deserialize, Infinite};
use crc::crc64::checksum_iso as checksum;
use memmap::{Mmap, Protection};
use memadvise::{Advice, advise};
use page_size::get as get_page_size;

use super::FILEARCO_MAGIC_NUMBER;
use file_data::FileData;

const VERSION_NUMBER: u64 = 1;

pub struct FileArco {
    inner: Arc<Inner>,
}

impl FileArco {
    pub fn new(path: &Path) -> Result<Self> {
        const U64S: usize = 8; // constant of mem::size_of::<u64>()
        const NUM_TOP_FIELDS: u64 = 4;

        let map = Mmap::open_path(path, Protection::Read)?;

        if map.len() < (NUM_TOP_FIELDS as usize) * U64S {
            return Err(Error::new(ErrorKind::InvalidData, "File too small for FileArchoV1!"));
        }

        // Read in initial data.

        let magic_number: u64 = unsafe {
            let ptr = map.ptr().offset(0);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };

        if magic_number != FILEARCO_MAGIC_NUMBER {
            return Err(Error::new(ErrorKind::InvalidData, "Not FileArco archive!"));
        }

        let version_number: u64 = unsafe {
            let ptr = map.ptr().offset(U64S as isize);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };

        if version_number != 1 {
            return Err(Error::new(ErrorKind::InvalidData, "Not FileArcho v1 archive!"));
        }

        let header_length: u64 = unsafe {
            let ptr = map.ptr().offset((2 * U64S) as isize);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };

        let header_checksum: u64 = unsafe {
            let ptr = map.ptr().offset((3 * U64S) as isize);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };

        // Read in header.

        if map.len() < (NUM_TOP_FIELDS as usize) * U64S + (header_length as usize) {
            return Err(Error::new(ErrorKind::InvalidData,
                                  "File is too small for FileArcho v1 header!"));
        }

        let header: Header = unsafe {
            let ptr = map.ptr().offset(((NUM_TOP_FIELDS as usize) * U64S) as isize);
            let s = slice::from_raw_parts(ptr, header_length as usize);

            // Ensure entries table is valid.
            let test_checksum = checksum(&s);

            if test_checksum != header_checksum {
                return Err(Error::new(ErrorKind::InvalidData,
                                      "Header has been corrupted!"));
            }

            deserialize(s).unwrap()
        };

        let should_prefetch = header.page_size == (get_page_size() as u64);

        // Read in entries data.

        if map.len() < ((NUM_TOP_FIELDS as usize) * U64S +
                        (header_length as usize) +
                        (header.entries_length as usize)) {
            return Err(Error::new(ErrorKind::InvalidData,
                                  "File is too small for entries table!"));
        }

        let entries = unsafe {
            let offset = NUM_TOP_FIELDS * (U64S as u64) + header_length;
            let ptr = map.ptr().offset(offset as isize);
            let s = slice::from_raw_parts(ptr, header.entries_length as usize);

            // Ensure entries table is valid.
            let test_checksum = checksum(&s);

            if test_checksum != header.entries_checksum {
                return Err(Error::new(ErrorKind::InvalidData,
                                      "Entries table has been corrupted!"));
            }

            deserialize(s).unwrap()
        };

        Ok(FileArco {
            inner: Arc::new(Inner {
                file_offset: header.file_offset,
                entries: entries,
                should_prefetch: should_prefetch,
                map: map,
            })
        })
    }

    pub fn get(&self, file_path: &str) -> Option<FileRef> {
        if let Some(entry) = self.inner.entries.files.get(file_path) {
            let offset = (self.inner.file_offset + entry.offset) as isize;
            let address = unsafe { self.inner.map.ptr().offset(offset) };

            // Advise system to start loading file from disk if it is not
            // already present.
            // NOTE: We only do this if the page size is 4 KiB.
            if self.inner.should_prefetch {
                advise(address as *mut (),
                       entry.aligned_length as usize,
                       Advice::WillNeed).ok().unwrap();
            }

            Some(FileRef {
                address: address,
                length: entry.length,
                aligned_length: entry.aligned_length,
                checksum: entry.checksum,
                should_advise: self.inner.should_prefetch,
                inner: self.inner.clone(),
            })
        }
        else {
            None
        }
    }
    
    pub fn make(file_data: FileData, out_path: &Path) -> Result<()> {
        const U64S: usize = 8; // constant of mem::size_of::<u64>()
        const NUM_TOP_FIELDS: u64 = 4;

        // Create output directories if they do not already exist.
        #[allow(unused_variables)]
        let res = match out_path.parent() {
            Some(parent) => create_dir_all(&parent),
            None => Ok(()),
        }?;

        let base_path = file_data.path();
   
        // Create entries table and serialize it.
        let entries = Entries::new(file_data)?;
        let entries_encoded: Vec<u8> = serialize(&entries, Infinite).unwrap();

        // Create header and serialize it.
        let header = Header::new(get_page_size() as u64,
                                 NUM_TOP_FIELDS * (U64S as u64),
                                 entries_encoded.len() as u64,
                                 checksum(&entries_encoded));
        let header_encoded = serialize(&header, Infinite).unwrap();
  
        // Create output archive.
        let mut out_file = File::create(out_path)?;

        // Write file identification number to archive.
        let magic_number = FILEARCO_MAGIC_NUMBER;
        let magic_number_encoded = unsafe {
            transmute::<u64, [u8; U64S]>(magic_number)
        };
        out_file.write_all(&magic_number_encoded)?;

        // Write version number to archive.
        let version_number = VERSION_NUMBER;
        let version_number_encoded = unsafe {
            transmute::<u64, [u8; U64S]>(version_number)
        };
        out_file.write_all(&version_number_encoded)?;

        // Write header length to archive.
        let header_length = header_encoded.len() as u64;
        let header_length_encoded = unsafe {
            transmute::<u64, [u8; U64S]>(header_length)
        };
        out_file.write_all(&header_length_encoded)?;

        // Write header checksum to archive.
        let header_checksum = checksum(&header_encoded);
        let header_checksum_encoded = unsafe {
            transmute::<u64, [u8; U64S]>(header_checksum)
        };
        out_file.write_all(&header_checksum_encoded)?;

        // Write serialized header to archive.
        out_file.write_all(&header_encoded)?;
        
        // Write out serialized entries table.
        out_file.write_all(&entries_encoded)?;

        // Pad archive with zeros to ensure files begin at a multiple of `page_size`.
        let padding_length = header.file_offset - (NUM_TOP_FIELDS * (U64S as u64) +
                                                   header_length +    
                                                   header.entries_length);
        let padding: Vec<u8> = vec![0u8; padding_length as usize];

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

#[allow(dead_code)]
pub struct FileRef {
    address: *const u8,
    length: u64,
    aligned_length: u64,
    checksum: u64,
    should_advise: bool,
    // Holding a reference to the memory mapped file ensures it will not be
    // unmapped until we finish using it.
    inner: Arc<Inner>,
}

impl FileRef {
    pub fn is_valid(&self) -> bool {
        let sl = self.as_slice();

        let checksum_computed = checksum(sl);

        self.checksum == checksum_computed
    }
    
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.address, self.length as usize)
        }
    }

    pub fn len(&self) -> u64 {
        self.length
    }
}

impl Drop for FileRef {
    fn drop(&mut self) {
        if self.should_advise {
            advise(self.address as *mut (),
                   self.aligned_length as usize,
                   Advice::DontNeed).ok().unwrap();
        }
    }
}

struct Inner {
    file_offset: u64,
    entries: Entries,
    should_prefetch: bool,
    map: Mmap,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Header {
    page_size: u64,
    file_offset: u64,
    entries_length: u64,
    entries_checksum: u64,
}

impl Header {
    fn new(current_offset: u64,
           page_size: u64,
           entries_length: u64,
           entries_checksum: u64) -> Self {
        // Serialize test struct to determine `file_offset`.
        let test_header = Header {
            page_size: page_size,
            file_offset: 0,
            entries_length: entries_length,
            entries_checksum: entries_checksum,
        };
        let test_header_encoded = serialize(&test_header, Infinite).unwrap();

        let file_offset = get_aligned_length(current_offset +
                                             (test_header_encoded.len() as u64) +
                                             entries_length);

        Header {
            page_size: page_size,
            file_offset: file_offset,
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
    fn new(file_data: FileData) -> Result<Self> {
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

        Ok(Entries {
            files: files 
        })
    }
}

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
    use std::fs::remove_file;
    
    use super::super::file_data::FileDatum;
    use super::*;

    fn get_file_data_stub(base_path: &Path) -> Result<FileData> {
        if base_path != Path::new("testarchives/simple") {
            return Err(Error::new(ErrorKind::Other,
                                  "Invalid base_path for test!"));
        }
        
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
            base_path.to_path_buf(),
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
        let entries = Entries::new(file_data).ok().unwrap();

        let simple = get_simple();

        for name in simple.iter() {
            assert!(entries.files.contains_key(name));
        }
    }

    #[test]
    fn test_v1_filearco_make() {
        let archive_path = Path::new("tmptest/tmparch.fac");

        // Remove test archive file from previous run of unit tests, if it exists.
        match remove_file(&archive_path) {
            _ => {},
        }

        let file_data = get_file_data_stub(&Path::new("testarchives/simple")).ok().unwrap();

        FileArco::make(file_data, &archive_path).unwrap();
    }

    #[test]
    fn test_v1_filearco_new() {
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let simple = get_simple();

        match FileArco::new(&archive_path) {
            Ok(archive) => {
                for name in simple.iter() {
                    assert!(archive.inner.entries.files.contains_key(name));
                }
            },
            Err(err) => {
                println!("{:?}", err);
                assert!(false); },
        }
    }

    #[test]
    fn test_v1_filearco_get() {
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let archive = FileArco::new(&archive_path).ok().unwrap();

        let simple = get_file_data_stub(Path::new("testarchives/simple")).ok().unwrap();
        let svec = simple.into_vec();

        for entry in svec.iter() {
            if let Some(fileref) = archive.get(entry.name().as_ref()) {
                assert_eq!(fileref.len(), entry.len());
                assert!(fileref.is_valid());
            }
            else {
                assert!(false);
            }
        }
    }

    #[test]
    fn test_v1_fileentry_as_slice() {
        let dir_path = Path::new("testarchives/simple");
        let archive_path = Path::new("testarchives/simple_v1.fac");
        let archive = FileArco::new(&archive_path).ok().unwrap();

        let simple = get_file_data_stub(&dir_path).ok().unwrap();
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
}
