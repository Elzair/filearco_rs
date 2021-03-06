#[macro_use]
extern crate clap;
extern crate filearco;

use std::error::Error;
use std::fs::File;
use std::io;
use std::process::exit;

fn main() {
    // let args = env::args().collect::<Vec<_>>();
    let matches = clap_app!(myapp =>
                            (version: "1.0")
                            (author: "Philip Woods <elzairthesorcerer@gmail.com>")
                            (about: "Archives FileArco files")
                            (@arg DIRPATH: +required "Path to directory to archive")
                            (@arg ARCHIVEVERSION: -v --archive_version +takes_value "Specify version of FileArco format to create")
                            (@arg FILEPATH: -p --path +takes_value "Write to FILEPATH instead of stdout")).get_matches();
    
    let dirpath = matches.value_of("DIRPATH").unwrap();
    let archive_version = matches.value_of("ARCHIVEVERSION").unwrap_or("1");

    if archive_version != "1" {
        println!("Invalid FileArco version: {}", archive_version);
        exit(-1);
    }

    let file_data = match filearco::get_file_data(dirpath) {
        Ok(data) => data,
        Err(err) => {
            // panic!(err.to_string())
            println!("{}", err.description());
            exit(-2);
        }
    };

    let handle = match matches.value_of("FILEPATH") {
        Some(file_path) => {
            match File::create(file_path) {
                Ok(handle) => Box::new(handle) as Box<io::Write>,
                Err(err) => {
                    println!("{}", err.description());
                    exit(-3);
                },
            }
        },
        None => {
            Box::new(io::stdout()) as Box<io::Write>
        },
    };

    match filearco::v1::FileArco::make(file_data, handle) {
        Ok(_) => {
            exit(0);
        },
        Err(err) => {
            println!("{}", err.description());
            exit(-4);
        }
    }
}
