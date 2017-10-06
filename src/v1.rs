use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::path::Path;
use std::result::Result as StdResult;

use bincode::{serialize, deserialize, Infinite};
use memmap::{Mmap, Protection};
use serde::{Serialize, Serializer};
use walkdir::WalkDir;

use super::FILEARCO_MAGIC_NUMBER;

pub struct FileArco {
    magic_number: u64,
    version_number: u64,
    header: Header,
}

impl FileArco {
    // pub fn new(path: &Path) -> Result<Self> {
    //     let file_map = Mmap::open_path(path, Protection::Read)?;

        
    // }
    
    pub fn make(base_path: &Path, out_path: &Path) -> Result<()> {
        let full_base_path = base_path.canonicalize()?;
        
        // Create output archive
        let mut out_file = File::create(out_path)?;

        // Create file header and write it to beginning of file.
        let header = Header::new(full_base_path.as_path())?;

        let header_encoded: Vec<u8> = serialize(&header, Infinite).unwrap();
        out_file.write_all(&header_encoded)?;

        // Pad header with zeros to ensure files began at a multiple of 4096.
        let padding_length = (header.file_offset as usize) - header_encoded.len();
        let padding: Vec<u8> = vec![0u8; padding_length];

        out_file.write_all(&padding)?;

        // Began writing files to archive.
        for (path, entry) in &header.files {
            let full_path = base_path.to_path_buf().join(Path::new(&path));
            let mut in_file = File::open(full_path)?;
            let mut buffer = Vec::<u8>::with_capacity(entry.length as usize); 
            in_file.read_to_end(&mut buffer)?;
            out_file.write_all(&buffer)?;
            
            let padding_length = (entry.aligned_length as usize) -
                (entry.length as usize);
            let padding: Vec<u8> = vec![0u8; padding_length];
            out_file.write_all(&padding)?;
        }
        
        Ok(())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Header {
    file_offset: u64,
    #[serde(serialize_with = "ordered_map")]
    files: HashMap<String, Entry>,
}

impl Header {
    fn new(base_path: &Path) -> Result<Self> {
        // Create test `Header` to get the size of the real `Header`.
        let file_data = get_file_data(base_path)?;
        let num_files = file_data.len();
        let mut files = HashMap::new();

        for datum in file_data {
            files.insert(datum.0,
                         Entry {
                             offset: 0,
                             length: datum.1,
                             aligned_length: get_aligned_length(datum.1),
                         });
        }

        let test = Header {
            file_offset: 0,
            files: files 
        };
        
        let encoded: Vec<u8> = serialize(&test, Infinite).unwrap();
        let encoded_len = encoded.len();
        let file_offset: u64 = get_aligned_length(encoded_len as u64);

        // Create real `Header`.
        let file_data = get_file_data(base_path)?;

        if file_data.len() != num_files {
            return Err(Error::new(ErrorKind::Other, "Filesystem changed!"));
        }
        
        let mut files = HashMap::new();
        let mut offset = file_offset;

        for datum in file_data {
            let aligned_length = get_aligned_length(datum.1);
            
            files.insert(datum.0,
                         Entry {
                             offset: offset,
                             length: datum.1,
                             aligned_length: aligned_length,
                         });

            offset = offset + aligned_length;
        }

        let header = Header {
            file_offset: file_offset,
            files: files,
        };

        let encoded: Vec<u8> = serialize(&test, Infinite).unwrap();
        if encoded.len() != encoded_len {
            return Err(Error::new(ErrorKind::Other, "Filesystem changed!"));
        }
        
        Ok(header)
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
    
    let mut file_names = FileData::new();

    for entry in WalkDir::new(base_path) {
        if let Ok(ent) = entry {
            if ent.file_type().is_file() {
                let file_path = ent.path().to_path_buf()
                    .strip_prefix(&base_path)
                    .unwrap().to_path_buf();
                let length = ent.metadata().ok().unwrap().len();

                // We only support valid UTF-8 file paths.
                if let Some(p) = file_path.to_str() {
                    file_names.push((
                        String::from(p),
                        length,
                    ));
                }
            }
        }
        else {
            return Err(Error::new(ErrorKind::Other, "Walk directory failed!"));
        }
    }

    Ok(file_names)
}

fn ordered_map<S>(value: &HashMap<String, Entry>, serializer: S)
                  -> StdResult<S::Ok, S::Error>
    where S: Serializer
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
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

        let header = Header::new(&path).ok().unwrap();

        println!("Offset: {}", header.file_offset);

        for key in header.files.keys() {
            println!("{}", key);
        }
    }
}
