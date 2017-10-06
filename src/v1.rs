use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::mem;
use std::slice;
use std::path::Path;

use bincode::{serialize, deserialize, Infinite};
use memmap::{Mmap, Protection};
use walkdir::WalkDir;

use super::FILEARCO_MAGIC_NUMBER;

const VERSION_NUMBER: u64 = 1;

pub struct FileArco {
    magic_number: u64,
    version_number: u64,
    file_offset: u64,
    entries_length: u64,
    entries: Entries,
    map: Mmap,
}

impl FileArco {
    pub fn new(path: &Path) -> Result<Self> {
        // let (magic_number, version_number): (u64, u64) = {
        //     let file = File::open(path)?;

        //     let magic_number_buf: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        //     file.read(&mut magic_number_buf[..])?;
        //     let magic_number: u64 = unsafe {
        //         mem::transmute::<[u8; 8], u64>(magic_number_buf)
        //     };

        //     let version_number_buf: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        //     file.read(&mut version_number_buf[..])?;
        //     let version_number: u64 = unsafe {
        //         mem::transmute::<[u8; 8], u64>(version_number_buf)
        //     };

        //     (magic_number, version_number)
        // };

        let map = Mmap::open_path(path, Protection::Read)?;

        if map.len() < 4 * mem::size_of::<u64>() {
            return Err(Error::new(ErrorKind::InvalidData, "File too small for header!"));
        }

        // Read in header data.

        let magic_number: u64 = unsafe {
            let ptr = map.ptr().offset(0);
            let s = mem::transmute::<*const u8, &[u8; 8]>(ptr);
            mem::transmute_copy::<[u8; 8], u64>(s)
        };

        if magic_number != FILEARCO_MAGIC_NUMBER {
            return Err(Error::new(ErrorKind::InvalidData, "Not FileArco archive!"));
        }

        let version_number: u64 = unsafe {
            let ptr = map.ptr().offset(mem::size_of::<u64>() as isize);
            let s = mem::transmute::<*const u8, &[u8; 8]>(ptr);
            mem::transmute_copy::<[u8; 8], u64>(s)
        };

        if magic_number != FILEARCO_MAGIC_NUMBER {
            return Err(Error::new(ErrorKind::InvalidData, "Not Version 1!"));
        }

        let file_offset: u64 = unsafe {
            let ptr = map.ptr().offset((2 * mem::size_of::<u64>()) as isize);
            let s = mem::transmute::<*const u8, &[u8; 8]>(ptr);
            mem::transmute_copy::<[u8; 8], u64>(s)
        };

        let entries_length: u64 = unsafe {
            let ptr = map.ptr().offset((3 * mem::size_of::<u64>()) as isize);
            let s = mem::transmute::<*const u8, &[u8; 8]>(ptr);
            mem::transmute_copy::<[u8; 8], u64>(s)
        };

        // Read in entries data.

        if map.len() < 4 * mem::size_of::<u64>() + (entries_length as usize) {
            return Err(Error::new(ErrorKind::InvalidData, "File too small for entries!"));
        }

        let entries = unsafe {
            let ptr = map.ptr().offset((4 * mem::size_of::<u64>()) as isize);
            let s = slice::from_raw_parts(ptr, entries_length as usize);

            deserialize(s).unwrap()
        };

        Ok(FileArco {
            magic_number: magic_number,
            version_number: version_number,
            file_offset: file_offset,
            entries_length: entries_length,
            entries: entries,
            map: map,
        })
    }
    
    pub fn make(base_path: &Path, out_path: &Path) -> Result<()> {
        let full_base_path = base_path.canonicalize()?;
        
        // Create output archive
        let mut out_file = File::create(out_path)?;

        // Write file identification number to archive.
        let magic_number = FILEARCO_MAGIC_NUMBER;
        let magic_number_encoded = unsafe {
            mem::transmute::<u64, [u8; 8]>(magic_number)
        };
        out_file.write_all(&magic_number_encoded)?;

        // Write version number to archive.
        let version_number = VERSION_NUMBER;
        let version_number_encoded = unsafe {
            mem::transmute::<u64, [u8; 8]>(version_number)
        };
        out_file.write_all(&version_number_encoded)?;

        // Create entries table and serialize it.
        let entries = Entries::new(full_base_path.as_path())?;
        let entries_encoded: Vec<u8> = serialize(&entries, Infinite).unwrap();

        // Write size of entries table (in bytes) to archive.
        let entries_length = entries_encoded.len() as u64;
        let entries_length_encoded = unsafe {
            mem::transmute_copy::<u64, [u8; 8]>(&entries_length)
        };
        out_file.write_all(&entries_length_encoded)?;

        // Write offset to first file (in bytes) to archive.
        let file_offset = get_aligned_length(4 * 8 + entries_length);
        let file_offset_encoded = unsafe {
            mem::transmute_copy::<u64, [u8; 8]>(&file_offset)
        };
        out_file.write_all(&file_offset_encoded)?;

        // Write out serialized entries table.
        out_file.write_all(&entries_encoded)?;

        // Pad archive with zeros to ensure files begin at a multiple of 4096.
        let padding_length = file_offset - (4 * 8 + entries_length);
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

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Entries {
    files: HashMap<String, Entry>,
}

impl Entries {
    fn new(base_path: &Path) -> Result<Self> {
        // Create test `Header` to get the size of the real `Header`.
        let file_data = get_file_data(base_path)?;
        let num_files = file_data.len();
        let mut files = HashMap::new();
        let mut offset = 0;

        for datum in file_data {
            let aligned_length = get_aligned_length(datum.1);

            files.insert(datum.0,
                         Entry {
                             offset: 0,
                             length: datum.1,
                             aligned_length: aligned_length,
                         });

            offset = offset + aligned_length;
        }

        Ok(Entries {
            files: files 
        })
        
        // let encoded: Vec<u8> = serialize(&test, Infinite).unwrap();
        // let encoded_len = encoded.len();
        // let file_offset: u64 = get_aligned_length(encoded_len as u64);

        // // Create real `Header`.
        // let file_data = get_file_data(base_path)?;

        // if file_data.len() != num_files {
        //     return Err(Error::new(ErrorKind::Other, "Filesystem changed!"));
        // }
        
        // let mut files = HashMap::new();
        // let mut offset = 0;

        // for datum in file_data {
        //     let aligned_length = get_aligned_length(datum.1);
            
        //     files.insert(datum.0,
        //                  Entry {
        //                      offset: offset,
        //                      length: datum.1,
        //                      aligned_length: aligned_length,
        //                  });

        //     offset = offset + aligned_length;
        // }

        // let header = Header {
        //     magic_number: FILEARCO_MAGIC_NUMBER,
        //     version_number: 1,
        //     file_offset: file_offset,
        //     files: files,
        // };

        // let encoded: Vec<u8> = serialize(&test, Infinite).unwrap();
        // if encoded.len() != encoded_len {
        //     return Err(Error::new(ErrorKind::Other, "Filesystem changed!"));
        // }
        
        // Ok(header)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Entry {
    offset: u64,
    length: u64,
    aligned_length: u64,
}

type FileData = Vec<(String, u64)>;
    
fn get_file_data(base_path: &Path) -> Result<FileData> {
    if !base_path.is_dir() {
        return Err(Error::new(ErrorKind::InvalidInput, "Not directory!"));
    }
    
    let mut file_data = FileData::new();

    for entry in WalkDir::new(base_path) {
        if let Ok(ent) = entry {
            if ent.file_type().is_file() {
                let file_path = ent.path().to_path_buf()
                    .strip_prefix(&base_path)
                    .unwrap().to_path_buf();
                let length = ent.metadata().ok().unwrap().len();

                // We only support valid UTF-8 file paths.
                if let Some(p) = file_path.to_str() {
                    file_data.push((
                        String::from(p),
                        length,
                    ));
                }
                else {
                    return Err(Error::new(ErrorKind::Other, "Non UTF-8 filename detected!"));
                }
            }
        }
        else {
            return Err(Error::new(ErrorKind::Other, "Walk directory failed!"));
        }
    }

    Ok(file_data)
}

/// This function returns the smallest multiple of 2^12 (i.e. 4096)
/// greater than or equal to the given length.
///
/// # Arguments
///
/// * length - the input number
#[inline]
fn get_aligned_length(length: u64) -> u64 {
    const FOUR_KIB: u64 = 4096;

    (length + (FOUR_KIB-1)) & !(FOUR_KIB-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_rounded_length() {
        assert_eq!(get_aligned_length(0), 0);
        assert_eq!(get_aligned_length(4096), 4096);
        assert_eq!(get_aligned_length(4096+1), 2 * 4096);
        assert_eq!(get_aligned_length(2*4096 - 1), 2 * 4096);
    }

    #[test]
    fn test_get_file_data() {
        let path = Path::new("src").canonicalize().ok().unwrap();

        let file_data = get_file_data(&path).ok().unwrap();

        assert_eq!(file_data.len(), 2);
    }

    // #[test]
    // fn test_print_files() {
    //     let path = Path::new("/home/elzair/tmp/test1");

    //     let file_data = get_file_data(&path);

    //     for datum in file_data {
    //         println!("{}", datum.0);
    //     }
    // }

    #[test]
    fn test_header_v1_new() {
        let path = Path::new("/home/elzair/tmp/test1");

        let entries = Entries::new(&path).ok().unwrap();

        for key in entries.files.keys() {
            println!("{}", key);
        }
    }
}
