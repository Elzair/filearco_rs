use std::fs::File;
use std::io::prelude::*;
use std::io::{Result, Error, ErrorKind};
use std::path::{Path, PathBuf};

use crc::crc64::checksum_iso as checksum;
use walkdir::WalkDir;
    
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
/// let file_data = filearco::get_file_data(&path).unwrap();
/// ```
pub fn get(base_path: &Path) -> Result<FileData> {
    if !base_path.is_dir() {
        return Err(Error::new(ErrorKind::InvalidInput,
                              format!(
                                  "Not directory: {}",
                                  base_path.to_string_lossy()
                              )));
    }
    
    let full_base_path = base_path.canonicalize()?;

    let mut file_data = Vec::<FileDatum>::new();

    for entry in WalkDir::new(&full_base_path) {
        match entry {
            Ok(ent) => {
                if ent.file_type().is_file() {
                    let full_path = ent.path().to_path_buf();
                    let file_path = full_path.strip_prefix(&full_base_path)
                        .unwrap().to_path_buf();
                    let length = ent.metadata().ok().unwrap().len();

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
                        return Err(Error::new(ErrorKind::Other,
                                              format!(
                                                  "Non UTF-8 filename detected: {}",
                                                  file_path.to_string_lossy()
                                              )));
                    }
                }
            },
            Err(err) => {
                if let Some(cycle_error_path) = err.loop_ancestor() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!(
                                              "Cycle detected for {}",
                                              cycle_error_path.to_string_lossy()
                                          )));
                }
                if let Some(general_error_path) = err.path() {
                    return Err(Error::new(ErrorKind::Other,
                                          format!(
                                              "Error reading {}",
                                              general_error_path.to_string_lossy()
                                          )));
                }

                return Err(Error::new(ErrorKind::Other,
                                      format!(
                                          "Walk directory failed at depth {}",
                                          err.depth()
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

    pub fn path(&self) -> PathBuf {
        self.base_path.clone()
    }
    
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn into_vec(self) -> Vec<FileDatum> {
        self.data
    }
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

        let path = Path::new("testarchives/reqchandocs");
        
        let file_data = get(&path).ok().unwrap();

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
