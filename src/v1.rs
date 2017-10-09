use std::collections::BTreeMap;
use std::fs::{File, create_dir_all};
use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::mem::{transmute, transmute_copy};
use std::slice;
use std::sync::Arc;
use std::path::Path;

use bincode::{serialize, deserialize, Infinite};
use memmap::{Mmap, Protection};
use memadvise::{Advice, advise};
use page_size::get as get_page_size;
use walkdir::WalkDir;

use super::FILEARCO_MAGIC_NUMBER;

const VERSION_NUMBER: u64 = 1;

pub struct FileArco {
    inner: Arc<Inner>,
}

impl FileArco {
    pub fn new(path: &Path) -> Result<Self> {
        const U64S: usize = 8; // constant of mem::size_of::<u64>()
        const NUM_HEADER_FIELDS: u64 = 5;

        let map = Mmap::open_path(path, Protection::Read)?;

        if map.len() < (NUM_HEADER_FIELDS as usize) * U64S {
            return Err(Error::new(ErrorKind::InvalidData, "File too small for header!"));
        }

        // Read in header data.

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

        let page_size: u64 = unsafe {
            let ptr = map.ptr().offset((2 * U64S) as isize);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };

        let should_prefetch = page_size == (get_page_size() as u64);
 
        let entries_length: u64 = unsafe {
            let ptr = map.ptr().offset((3 * U64S) as isize);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };
       
        let file_offset: u64 = unsafe {
            let ptr = map.ptr().offset((4 * U64S) as isize);
            let s = transmute::<*const u8, &[u8; U64S]>(ptr);
            transmute_copy::<[u8; U64S], u64>(s)
        };
        println!("File Offset: {}", file_offset);

        // Read in entries data.

        if map.len() < (NUM_HEADER_FIELDS as usize) * U64S + (entries_length as usize) {
            return Err(Error::new(ErrorKind::InvalidData,
                                  "File too small for entries!"));
        }

        let entries = unsafe {
            let ptr = map.ptr().offset(((NUM_HEADER_FIELDS as usize) * U64S) as isize);
            let s = slice::from_raw_parts(ptr, entries_length as usize);

            deserialize(s).unwrap()
        };

        Ok(FileArco {
            inner: Arc::new(Inner {
                file_offset: file_offset,
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
            let length = entry.length;
            let aligned_length = entry.aligned_length;

            // Advise system to start loading file from disk if it is not
            // already present.
            // NOTE: We only do this if the page size is 4 KiB.
            if self.inner.should_prefetch {
                advise(address as *mut (),
                       aligned_length as usize,
                       Advice::WillNeed).ok().unwrap();
            }

            Some(FileRef {
                address: address,
                length: length,
                aligned_length: aligned_length,
                offset: offset,
                should_advise: self.inner.should_prefetch,
                inner: self.inner.clone(),
            })
        }
        else {
            None
        }
    }
    
    pub fn make(base_path: &Path, out_path: &Path) -> Result<()> {
        const U64S: usize = 8; // constant of mem::size_of::<u64>()
        const NUM_HEADER_FIELDS: u64 = 5;

        // Create output directories if they do not already exist.
        #[allow(unused_variables)]
        let res = match out_path.parent() {
            Some(parent) => create_dir_all(&parent),
            None => Ok(()),
        }?;
        
        let full_base_path = base_path.canonicalize()?;

        // Create output archive
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

        // Write memory page size of current system to archive.
        let page_size = get_page_size() as u64;
        let page_size_encoded = unsafe {
            transmute::<u64, [u8; U64S]>(page_size)
        };
        out_file.write_all(&page_size_encoded)?;

        // Create entries table and serialize it.
        let entries = Entries::new(full_base_path.as_path())?;
        let entries_encoded: Vec<u8> = serialize(&entries, Infinite).unwrap();

        // Write size of entries table (in bytes) to archive.
        let entries_length = entries_encoded.len() as u64;
        let entries_length_encoded = unsafe {
            transmute_copy::<u64, [u8; U64S]>(&entries_length)
        };
        out_file.write_all(&entries_length_encoded)?;

        // Write offset to first file (in bytes) to archive.
        let file_offset = get_aligned_length(NUM_HEADER_FIELDS * (U64S as u64) +
                                             entries_length);
        let file_offset_encoded = unsafe {
            transmute_copy::<u64, [u8; U64S]>(&file_offset)
        };
        out_file.write_all(&file_offset_encoded)?;
        println!("File Offset: {}", file_offset);

        // Write out serialized entries table.
        out_file.write_all(&entries_encoded)?;

        // Pad archive with zeros to ensure files begin at a multiple of `page_size`.
        let padding_length = file_offset - (NUM_HEADER_FIELDS * (U64S as u64) +
                                            entries_length);
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

pub struct FileRef {
    address: *const u8,
    length: u64,
    aligned_length: u64,
    offset: isize,
    should_advise: bool,
    // Holding a reference to the memory mapped file ensures it will not be
    // unmapped until we finish using it.
    inner: Arc<Inner>,
}

impl FileRef {
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
struct Entries {
    files: BTreeMap<String, Entry>,
}

impl Entries {
    fn new(base_path: &Path) -> Result<Self> {
        // Create test `Header` to get the size of the real `Header`.
        let file_data = get_file_data(base_path)?;
        let mut files = BTreeMap::new();

        for datum in file_data {
            let aligned_length = get_aligned_length(datum.1);

            files.insert(datum.0,
                         Entry {
                             offset: 0,
                             length: datum.1,
                             aligned_length: aligned_length,
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
}

type FileData = Vec<(String, u64)>;
    
fn get_file_data(base_path: &Path) -> Result<FileData> {
    if !base_path.is_dir() {
        return Err(Error::new(ErrorKind::InvalidInput, "Not directory!"));
    }
    
    let mut file_data = FileData::new();

    for entry in WalkDir::new(&base_path) {
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
    let page_size = get_page_size() as u64;

    // Assume memory page size is a power of 2.
    (length + (page_size-1)) & !(page_size-1)
}

#[cfg(test)]
mod tests {
    use std::fs::remove_file;
    
    use super::*;

    fn get_reqchan_docs() -> Vec<String> {
        let mut v = Vec::<String>::new();

        v.push(String::from("implementors/core/clone/trait.Clone.js"));
        v.push(String::from("implementors/core/fmt/trait.Debug.js"));
        v.push(String::from("implementors/core/ops/trait.Drop.js"));
        v.push(String::from("reqchan/RequestContract.t.html"));
        v.push(String::from("reqchan/Requester.t.html"));
        v.push(String::from("reqchan/Responder.t.html"));
        v.push(String::from("reqchan/ResponseContract.t.html"));
        v.push(String::from("reqchan/TryReceiveError.t.html"));
        v.push(String::from("reqchan/TryRequestError.t.html"));
        v.push(String::from("reqchan/TryRespondError.t.html"));
        v.push(String::from("reqchan/channel.v.html"));
        v.push(String::from("reqchan/enum.TryReceiveError.html"));
        v.push(String::from("reqchan/enum.TryRequestError.html"));
        v.push(String::from("reqchan/enum.TryRespondError.html"));
        v.push(String::from("reqchan/fn.channel.html"));
        v.push(String::from("reqchan/index.html"));
        v.push(String::from("reqchan/sidebar-items.js"));
        v.push(String::from("reqchan/struct.RequestContract.html"));
        v.push(String::from("reqchan/struct.Requester.html"));
        v.push(String::from("reqchan/struct.Responder.html"));
        v.push(String::from("reqchan/struct.ResponseContract.html"));
        v.push(String::from("src/reqchan/lib.rs.html"));
        v.push(String::from("COPYRIGHT.txt"));
        v.push(String::from("FiraSans-LICENSE.txt"));
        v.push(String::from("FiraSans-Medium.woff"));
        v.push(String::from("FiraSans-Regular.woff"));
        v.push(String::from("Heuristica-Italic.woff"));
        v.push(String::from("Heuristica-LICENSE.txt"));
        v.push(String::from("LICENSE-APACHE.txt"));
        v.push(String::from("LICENSE-MIT.txt"));
        v.push(String::from("SourceCodePro-LICENSE.txt"));
        v.push(String::from("SourceCodePro-Regular.woff"));
        v.push(String::from("SourceCodePro-Semibold.woff"));
        v.push(String::from("SourceSerifPro-Bold.woff"));
        v.push(String::from("SourceSerifPro-LICENSE.txt"));
        v.push(String::from("SourceSerifPro-Regular.woff"));
        v.push(String::from("main.css"));
        v.push(String::from("main.js"));
        v.push(String::from("normalize.css"));
        v.push(String::from("rustdoc.css"));
        v.push(String::from("search-index.js"));

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
    fn test_v1_get_file_data() {
        let path = Path::new("testarchives/reqchandocs").canonicalize().ok().unwrap();
        let reqchan_docs = get_reqchan_docs();

        let file_data = get_file_data(&path).ok().unwrap();

        assert_eq!(file_data.len(), reqchan_docs.len());

        for name in reqchan_docs.iter() {
            let mut found = false;

            for dname in file_data.iter() {
                if *name == *dname.0 {
                    found = true;
                }
            }

            if !found {
                println!("{} not found!", *name);
            }
            assert!(found);
        }
    }

    #[test]
    fn test_v1_entries_new() {
        let path = Path::new("testarchives/reqchandocs");
        let reqchan_docs = get_reqchan_docs();

        let entries = Entries::new(&path).ok().unwrap();

        for name in reqchan_docs.iter() {
            assert!(entries.files.contains_key(name));
        }
    }

    #[test]
    fn test_v1_filearco_make() {
        let in_path = Path::new("testarchives/reqchandocs");
        let archive_path = Path::new("tmptest/tmparch.fac");

        // Remove test archive file from previous run of unit tests, if it exists.
        match remove_file(&archive_path) {
            _ => {},
        }

        FileArco::make(&in_path, &archive_path).unwrap();
    }

    #[test]
    fn test_v1_filearco_new() {
        let archive_path = Path::new("testarchives/reqchandocs_v1.fac");
        let reqchan_docs = get_reqchan_docs();

        match FileArco::new(&archive_path) {
            Ok(archive) => {
                for name in reqchan_docs.iter() {
                    assert!(archive.inner.entries.files.contains_key(name));
                }
            },
            Err(_) => { assert!(false); },
        }
    }

    #[test]
    fn test_v1_filearco_get() {
        let archive_path = Path::new("testarchives/reqchandocs_v1.fac");
        let reqchan_docs = get_reqchan_docs();
        let archive = FileArco::new(&archive_path).ok().unwrap();

        for name in reqchan_docs.iter() {
            match archive.get(name) {
                Some(_) => {},
                None => { assert!(false); }
            }
        }
    }

    #[test]
    fn test_v1_fileentry_len() {
        let base_path = String::from("testarchives/reqchandocs");
        let archive_path = Path::new("testarchives/reqchandocs_v1.fac");
        let reqchan_docs = get_reqchan_docs();
        let archive = FileArco::new(&archive_path).ok().unwrap();

        for name in reqchan_docs.into_iter() {
            let full_name = format!("{}/{}", &base_path, &name);
            let full_path = Path::new(&full_name);
            let length = full_path.metadata().ok().unwrap().len() as u64;
            
            let archived_file = archive.get(&name).unwrap();

            assert_eq!(length, archived_file.len());
            assert_eq!(get_aligned_length(length), archived_file.aligned_length);
        }
    }

    #[test]
    fn test_v1_fileentry_as_slice() {
        let base_path = String::from("testarchives/reqchandocs");
        let archive_path = Path::new("testarchives/reqchandocs_v1.fac");
        let reqchan_docs = get_reqchan_docs();
        let archive = FileArco::new(&archive_path).ok().unwrap();

        for name in reqchan_docs.into_iter() {
            let full_name = format!("{}/{}", &base_path, &name);
            let full_path = Path::new(&full_name);
            let length = full_path.metadata().ok().unwrap().len();

            println!("Testing: {}", &full_name);

            // Read in input file contents.
            let mut in_file = File::open(full_path).ok().unwrap();
            let mut contents1 = Vec::<u8>::with_capacity(length as usize); 
            in_file.read_to_end(&mut contents1).ok().unwrap();
            
            let archived_file = archive.get(&name).unwrap();
            let length2 = archived_file.len();

            assert_eq!(length, archived_file.as_slice().len() as u64);
            assert_eq!(length2, archived_file.as_slice().len() as u64);
            assert_eq!(contents1, archived_file.as_slice());

            // println!("Equaled: {}", &full_name);
        }
    }

    #[test]
    fn test_v1_simple() {
        let dir_path = String::from("testarchives/simple");
        let mut paths = Vec::<String>::new();
        paths.push(String::from("Cargo.toml"));
        paths.push(String::from("LICENSE-APACHE"));
        paths.push(String::from("LICENSE-MIT"));

        let archive_path = Path::new("tmptest/simple_v1.fac");

        // Remove test archive file from previous run of unit tests, if it exists.
        match remove_file(&archive_path) {
            _ => {},
        }

        let in_path = Path::new(&dir_path);
        FileArco::make(&in_path, &archive_path).unwrap();

        let archive = FileArco::new(&archive_path).ok().unwrap();

        for name in paths.into_iter() {
            let full_name = format!("{}/{}", &dir_path, &name);
            let path = Path::new(&full_name);
            let length = path.metadata().ok().unwrap().len();

            println!("Testing: {} {}", &name, length);

            // Read in input file contents.
            let mut in_file = File::open(&path).ok().unwrap();
            let mut contents1 = Vec::<u8>::with_capacity(length as usize); 
            in_file.read_to_end(&mut contents1).ok().unwrap();
            let sl1 = contents1.as_slice();
            // let s1 = unsafe { transmute::<&[u8], &str>(sl1) };
            
            let archived_file = archive.get(&name).unwrap();
            println!("Offset for {}: {}", name, archived_file.offset);
            let sl2 = archived_file.as_slice();
            // let s2 = unsafe { transmute::<&[u8], &str>(sl2) };

            // assert_eq!(s1, s2);
            assert_eq!(sl1, sl2);

            println!("Equaled: {}", &full_name);
        }
    }
}
