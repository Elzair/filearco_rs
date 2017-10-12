extern crate filearco;

use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use filearco::get_file_data;
use filearco::v1::FileArco;

#[test]
fn test_make_read_v1() {
    let dir_path = Path::new("testarchives/reqchandocs");
    let archive_path = Path::new("tmptest/make_read_v1_test.fac");

    let file_data = get_file_data(&dir_path).ok().unwrap();
    let file_data_copy = file_data.clone();

    FileArco::make(file_data, &archive_path).ok().unwrap();

    let archive = FileArco::new(&archive_path).ok().unwrap();

    let datums = file_data_copy.into_vec();

    for datum in datums.into_iter() {
        let fileref = archive.get(datum.name()).unwrap();

        assert_eq!(datum.len(), fileref.len());
        assert!(fileref.is_valid());

        let full_name = format!(
            "{}/{}",
            &dir_path.to_string_lossy(),
            &datum.name()
        );
        let full_path = Path::new(&full_name);
        let mut in_file = File::open(full_path).ok().unwrap();
        let mut contents = Vec::<u8>::with_capacity(datum.len() as usize); 
        in_file.read_to_end(&mut contents).ok().unwrap();

        assert_eq!(contents, fileref.as_slice());
    }
}
