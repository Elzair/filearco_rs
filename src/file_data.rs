//! This module contains a function `get()` to retrieve a list of all ordinary
//! files in a given directory hierarchy.
//!
//! # Example
//!
//! ```rust
//! extern crate filearco;
//!
//! use std::path::Path;
//!
//! let path = Path::new("testarchives/simple");
//! let file_data = filearco::get_file_data(path).unwrap();
//! ```

use std::convert::AsRef;
use std::error;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use crc::crc64::checksum_iso as checksum;
use walkdir::WalkDir;
    
use super::{Error, Result};

/// This function retrieves basic information (i.e. path, length and checksum)
/// of all files under a specific `base_path`.
///
/// **NOTE:** All file paths are relative to `base_dir`
///
/// # Arguments
///
/// * base_path - the path of a *directory* to list.
///
/// # Example
///
/// ```rust
/// extern crate filearco;
///
/// use std::path::Path;
///
/// let path = Path::new("testarchives/simple");
/// let file_data = filearco::get_file_data(path).unwrap();
/// ```
pub fn get<P: AsRef<Path>>(base_path: P) -> Result<FileData> {
    if !base_path.as_ref().is_dir() {
        return Err(Error::FileData(FileDataError::BasePathNotDirectory));
    }
    
    let full_base_path = base_path.as_ref().canonicalize()?;

    let mut file_data = Vec::<FileDatum>::new();

    for entry in WalkDir::new(&full_base_path) {
        let ent = entry?;

        if ent.file_type().is_file() {
            let full_path = ent.path().to_path_buf();
            let file_path = full_path.strip_prefix(&full_base_path)
                .unwrap().to_path_buf();
            let metadata = ent.metadata()?;
            let length = metadata.len();

            // We only support valid UTF-8 file paths.
            if let Some(p) = file_path.to_str() {
                // Compute checksum of file contents. 
                let mut in_file = File::open(full_path)?;
                let mut contents = Vec::<u8>::with_capacity(length as usize); 
                in_file.read_to_end(&mut contents)?;
                let contents_checksum = checksum(&contents); 

                file_data.push(FileDatum {
                    name: String::from(p),
                    length: length,
                    checksum: contents_checksum,
                });
            }
            else {
                return Err(Error::FileData(FileDataError::NonUtf8Filepath(
                    String::from(file_path.to_string_lossy())
                )));
            }
        }
    }

    Ok(FileData {
        base_path: full_base_path,
        data: file_data,
    })
}

/// This struct contains information on all the normal files in a given location.
#[derive(Clone)]
pub struct FileData {
    base_path: PathBuf,
    data: Vec<FileDatum>,
}

impl FileData {
    // This is needed for unit tests in v1.rs so the fields of
    // `FileData` do not have to be public.
    #[cfg(test)]
    pub fn new(base_path: PathBuf, data: Vec<FileDatum>) -> Self {
        FileData {
            base_path: base_path,
            data: data,
        }
    }

    /// This method returns the path of the indexed directory.
    pub fn path(&self) -> PathBuf {
        self.base_path.clone()
    }
    
    /// This method returns the number of files indexed.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// This method consumes this struct and returns a Vec of its contents.
    pub fn into_vec(self) -> Vec<FileDatum> {
        self.data
    }
}

/// Errors retrieving information on files
#[derive(Debug)]
pub enum FileDataError {
    /// Input path is not a directory
    BasePathNotDirectory,
    /// Non UTF-8 filename detected
    NonUtf8Filepath(String),
}

impl fmt::Display for FileDataError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FileDataError::BasePathNotDirectory => {
                write!(fmt, "Base path is not a directory")
            },
            FileDataError::NonUtf8Filepath(ref file_path) => {
                write!(fmt, "{}", file_path)
            },
        }
    }
}

impl error::Error for FileDataError {
    fn description(&self) -> &str {
        static BASE_PATH_NOT_DIRECTORY: &'static str = "Base path is not a directory";
        static NON_UTF8_FILE_PATH: &'static str = "Non-Utf8 file path detected";

        match *self {
            FileDataError::BasePathNotDirectory => {
                BASE_PATH_NOT_DIRECTORY
            },
            FileDataError::NonUtf8Filepath(_) => {
                NON_UTF8_FILE_PATH
            },
        }
    }

    fn cause(&self) -> Option<&error::Error> { None }
}

/// This struct contains basic information about a file.
#[derive(Clone)]
pub struct FileDatum {
    name: String,
    length: u64,
    checksum: u64,
}

impl FileDatum {
    // This is needed for unit tests in v1.rs so the fields of
    // `FileDatum` do not have to be public.
    #[cfg(test)]
    pub fn new(name: String, length: u64, checksum: u64) -> Self {
        FileDatum {
            name: name,
            length: length,
            checksum: checksum,
        }
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn len(&self) -> u64 {
        self.length
    }

    pub fn checksum(&self) -> u64 {
        self.checksum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn get_reqchan_docs() -> Vec<String> {
        let mut v = Vec::<String>::new();

        v.push(String::from("implementors\\core\\clone\\trait.Clone.js"));
        v.push(String::from("implementors\\core\\fmt\\trait.Debug.js"));
        v.push(String::from("implementors\\core\\ops\\trait.Drop.js"));
        v.push(String::from("reqchan\\RequestContract.t.html"));
        v.push(String::from("reqchan\\Requester.t.html"));
        v.push(String::from("reqchan\\Responder.t.html"));
        v.push(String::from("reqchan\\ResponseContract.t.html"));
        v.push(String::from("reqchan\\TryReceiveError.t.html"));
        v.push(String::from("reqchan\\TryRequestError.t.html"));
        v.push(String::from("reqchan\\TryRespondError.t.html"));
        v.push(String::from("reqchan\\channel.v.html"));
        v.push(String::from("reqchan\\enum.TryReceiveError.html"));
        v.push(String::from("reqchan\\enum.TryRequestError.html"));
        v.push(String::from("reqchan\\enum.TryRespondError.html"));
        v.push(String::from("reqchan\\fn.channel.html"));
        v.push(String::from("reqchan\\index.html"));
        v.push(String::from("reqchan\\sidebar-items.js"));
        v.push(String::from("reqchan\\struct.RequestContract.html"));
        v.push(String::from("reqchan\\struct.Requester.html"));
        v.push(String::from("reqchan\\struct.Responder.html"));
        v.push(String::from("reqchan\\struct.ResponseContract.html"));
        v.push(String::from("src\\reqchan\\lib.rs.html"));
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
    #[cfg(not(windows))]
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
    fn test_v1_get_file_data() {
        let reqchan_docs = get_reqchan_docs();

        // For some reason, Appveyor builds do not like the big archive.
        // Since all the other unit tests seem to work, and even this one
        // works on other systems, I just use the simpler directory.
        let path = Path::new("testarchives/reqchandocs");
        
        let file_data = get(path).ok().unwrap();

        let full_path = path.canonicalize().ok().unwrap();
        
        assert_eq!(full_path, file_data.path());
        assert_eq!(file_data.len(), reqchan_docs.len());

        let fdvec = file_data.into_vec();

        for name in reqchan_docs.iter() {
            let mut found = false;

            for dname in fdvec.iter() {
                if *name == *dname.name() {
                    found = true;
                }
            }

            if !found {
                println!("{} not found!", *name);
            }
            assert!(found);
        }
    }
}
