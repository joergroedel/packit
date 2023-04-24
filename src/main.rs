use clap::Parser;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::mem::{size_of, size_of_val};
use std::path::Path;
use std::vec::Vec;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of output file
    #[arg(short, long)]
    output: String,

    /// Directory to pack
    #[arg(short, long)]
    dir: String,
}

fn traverse_subdir(
    path: &Path,
    base_path: &str,
    prefix: &OsString,
    list: &mut Vec<OsString>,
) -> Result<(), std::io::Error> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();

            let name = OsString::from(p.to_str().unwrap());
            let mut full_name = prefix.clone();
            full_name.push("/");
            full_name.push(name.clone());

            if p.is_dir() {
                traverse_subdir(&p, base_path, &full_name, list)?;
            } else {
                let s = OsString::from(p.strip_prefix(base_path).unwrap());
                list.push(s);
            }
        }
    }

    Ok(())
}

fn traverse_path(path: &Path) -> Result<Vec<OsString>, std::io::Error> {
    let mut list: Vec<OsString> = Vec::new();

    let entries = fs::read_dir(path)?;
    let base_path = path.to_str().unwrap();

    for entry in entries {
        let entry = entry.unwrap();
        let p = entry.path();
        let name = OsString::from(p.to_str().unwrap());

        if p.is_dir() {
            traverse_subdir(&p, base_path, &name, &mut list)?;
        } else {
            let s = OsString::from(p.strip_prefix(base_path).unwrap());
            list.push(s);
        }
    }

    Ok(list)
}

struct PackItHeader {
    /// Header Magic (PKIT)
    magic: [u8; 4],
    /// Header Size
    header_size: u32,
}

impl PackItHeader {
    pub const fn new() -> Self {
        PackItHeader {
            magic: [0x50, 0x4b, 0x49, 0x54],
            header_size: size_of::<PackItHeader>() as u32,
        }
    }

    pub fn write(&self, file: &mut File) -> Result<(), std::io::Error> {
        file.write(&self.magic)?;

        let size: u32 = (size_of_val(&self.magic) + size_of_val(&self.header_size))
            .try_into()
            .unwrap();
        file.write_all(&size.to_le_bytes())?;

        Ok(())
    }
}

fn pack_file(out_file: &mut File, name: &OsString, base_path: &str) -> Result<(), std::io::Error> {
    let file_name = String::from(base_path) + "/" + name.to_str().unwrap();
    let mut in_file = File::open(file_name.as_str()).expect("Failed to open file for reading");
    let meta = in_file.metadata().expect("Failed to load file metadata");

    let filename_len: u16 = name.len().try_into().unwrap();

    // Write entry header
    // 2-byte entry type (1)
    out_file.write_all(&1u16.to_le_bytes())?;

    // Length of the filename which comes right after the header
    out_file.write_all(&filename_len.to_le_bytes())?;

    // Write length of file
    out_file.write_all(&meta.len().to_le_bytes())?;

    // Write file name
    out_file.write_all(name.to_str().unwrap().as_bytes())?;

    // Write file content
    let mut buf: [u8; 1024] = [0; 1024];
    loop {
        let result = in_file.read(&mut buf);
        if let Err(e) = result {
            if e.kind() == ErrorKind::Interrupted {
                continue;
            } else {
                return Err(e);
            }
        }

        let size = result.unwrap();
        if size == 0 {
            break;
        }

        out_file.write_all(&buf[..size])?;
    }

    Ok(())
}

fn main() {
    let args = Args::parse();

    let path = Path::new(&args.dir);

    let file_list = traverse_path(path).unwrap();

    for file in file_list.iter() {
        println!("File: {}", file.to_str().unwrap());
    }

    let header = PackItHeader::new();

    let mut file = File::create(args.output).unwrap();

    header
        .write(&mut file)
        .expect("Failed to write file header");

    for filename in file_list.iter() {
        pack_file(&mut file, filename, args.dir.as_str()).expect("Failed to pack file");
    }
}
