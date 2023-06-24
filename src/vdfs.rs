use core::fmt;
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    process::exit,
};

use chrono::{Datelike, Timelike};

#[allow(dead_code)]
#[derive(Debug)]
pub struct VDFSHeader {
    comment: [u8; 256],
    signature: [u8; 16],
    num_files: u32,
    num_entries: u32,
    timestamp: u32,
    size: u32,
    catalog_offset: u32,
    version: u32,
}

impl fmt::Display for VDFSHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let comment = String::from_utf8_lossy(&self.comment);
        let signature = String::from_utf8_lossy(&self.signature);

        writeln!(f, "Comment: {}", comment.trim_end_matches('\u{0}'))?;
        writeln!(f, "Signature: {}", signature.trim_end_matches('\u{0}'))?;
        writeln!(f, "Number of Files: {}", self.num_files)?;
        writeln!(f, "Number of Entries: {}", self.num_entries)?;
        writeln!(f, "Timestamp: {}", self.timestamp)?;
        writeln!(f, "Size: {}", self.size)?;
        writeln!(f, "Catalog Offset: {}", self.catalog_offset)?;
        writeln!(f, "Version: {}", self.version)?;

        Ok(())
    }
}

impl VDFSHeader {
    fn comment(&mut self, cmnt: &str) {
        self.comment[..cmnt.len()].copy_from_slice(cmnt.as_bytes());
    }
}

impl Default for VDFSHeader {
    fn default() -> Self {
        VDFSHeader {
            comment: [0x1A; 256],
            signature: [
                0x50, 0x53, 0x56, 0x44, 0x53, 0x43, 0x5F, 0x56, 0x32, 0x2E, 0x30, 0x30, 0x0A, 0x0D,
                0x0A, 0x0D,
            ], // PSVDSC_V2.00\n\r\n\r
            timestamp: get_current_dos_time(),
            num_files: 0,
            num_entries: 0,
            size: 0,
            catalog_offset: 0,
            version: 80,
        }
    }
}

fn get_current_dos_time() -> u32 {
    let mut time: u32 = 0;
    let curr = chrono::Utc::now();
    time |= ((curr.year() - 1980) as u32) << 25;
    time |= ((curr.month0() + 1) as u32) << 21;
    time |= (curr.day() as u32) << 16;
    time |= (curr.hour() as u32) << 11;
    time |= (curr.minute() as u32) << 5;
    time |= ((curr.second() / 2) as u32) << 0;

    time
}

enum EntryType {
    Dir = 0x80000000,
    LastFile = 0x40000000,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct VDFSCatalogEntry {
    name: [u8; 64],
    next_index: u32,
    size: u32,
    typ: u32,
    attributes: u32,
}
impl VDFSCatalogEntry {
    fn new(file_name: &str) -> VDFSCatalogEntry {
        let mut vdfs = VDFSCatalogEntry::default();
        vdfs.name[..file_name.len()].copy_from_slice(file_name.to_ascii_uppercase().as_bytes());
        vdfs
    }
    fn new_sized(file_name: &str, size: u64) -> VDFSCatalogEntry {
        let mut vdfs = VDFSCatalogEntry::default();
        vdfs.name[..file_name.len()].copy_from_slice(file_name.to_ascii_uppercase().as_bytes());
        vdfs.size = size as u32;
        vdfs
    }
}

impl fmt::Display for VDFSCatalogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = String::from_utf8_lossy(&self.name);

        writeln!(f, "Name: {}", name)?;
        writeln!(f, "Offset: {}", self.next_index)?;
        writeln!(f, "Size: {}", self.size)?;
        writeln!(f, "Type: {}", self.typ)?;
        // writeln!(f, "Attributes: {}", self.attributes)?;

        Ok(())
    }
}

impl Default for VDFSCatalogEntry {
    fn default() -> Self {
        VDFSCatalogEntry {
            name: [
                0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
                0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
                0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
                0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
                0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
            ],
            next_index: 0,
            size: 0,
            typ: 0,
            attributes: 0,
        }
    }
}

#[derive(Debug)]
pub struct VDFS {
    pub header: VDFSHeader,
    pub catalog: Vec<VDFSCatalogEntry>,
    files: Vec<VDFSCatalogEntry>,
    pub data: Vec<u8>,
    pub curr_pos: u32,
}

impl fmt::Display for VDFS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "VDFS Header:")?;
        writeln!(f, "{}", self.header)?;

        writeln!(f, "VDFS Catalog:")?;
        for (i, entry) in self.catalog.iter().enumerate() {
            writeln!(f, "{i}\n{}\n", entry)?;
        }
        for (i, entry) in self.files.iter().enumerate() {
            writeln!(f, "{}\n{}\n", i + self.catalog.len(), entry)?;
        }

        Ok(())
    }
}

impl VDFS {
    pub fn new(path: &PathBuf) -> Self {
        let mut vdfs = VDFS {
            header: VDFSHeader::default(),
            catalog: Vec::new(),
            files: Vec::new(),
            data: Vec::new(),
            curr_pos: 0,
        };

        vdfs.build_catalog(&path.to_string_lossy().into_owned());
        vdfs.calculate_data_size();
        vdfs
    }

    // This could be done elegantly with serde, but I don't know how to use it :kekw:
    pub fn save_to_file(&self, output_file: &PathBuf) -> Result<(), std::io::Error> {
        let file = File::create(output_file)?;

        let mut buf_writer = BufWriter::new(file);

        buf_writer.write(&self.header.comment)?;
        buf_writer.write(&self.header.signature)?;
        buf_writer.write(&self.header.num_files.to_le_bytes())?;
        buf_writer.write(&self.header.num_entries.to_le_bytes())?;
        buf_writer.write(&self.header.timestamp.to_le_bytes())?;
        buf_writer.write(&self.header.size.to_le_bytes())?;
        buf_writer.write(&self.header.catalog_offset.to_le_bytes())?;
        buf_writer.write(&self.header.version.to_le_bytes())?;

        for c in &self.catalog {
            buf_writer.write(&c.name)?;
            buf_writer.write(&c.next_index.to_le_bytes())?;
            buf_writer.write(&c.size.to_le_bytes())?;
            buf_writer.write(&c.typ.to_le_bytes())?;
            buf_writer.write(&c.attributes.to_le_bytes())?;
        }

        for f in &self.files {
            buf_writer.write(&f.name)?;
            buf_writer.write(&f.next_index.to_le_bytes())?;
            buf_writer.write(&f.size.to_le_bytes())?;
            buf_writer.write(&f.typ.to_le_bytes())?;
            buf_writer.write(&f.attributes.to_le_bytes())?;
        }

        buf_writer.write(&self.data)?;

        buf_writer.flush()?;

        Ok(())
    }

    pub fn build_catalog(&mut self, base_dir: &str) {
        let mut dir_queue = VecDeque::new();
        dir_queue.push_back((1, base_dir.to_owned())); // Add the root directory to the queue

        let mut offset = 0;
        while let Some((level, dir_path)) = dir_queue.pop_front() {
            match fs::read_dir(&dir_path) {
                Ok(entries) => {
                    let mut paths: Vec<_> = entries.collect();

                    paths.sort_by_key(|dir| !dir.as_ref().unwrap().path().is_dir()); // Sort = dirs then files

                    // Last index for the directory portion
                    let mut last_dir_index = 0;
                    if let Some(index) = paths
                        .iter()
                        .rposition(|entry| entry.as_ref().unwrap().path().is_dir())
                    {
                        last_dir_index = index;
                    }

                    for (entry_index, entry) in paths.iter().enumerate() {
                        if let Ok(entry) = entry {
                            let path = entry.path();

                            if path.is_dir() {
                                dir_queue.push_back((offset, path.to_string_lossy().to_string()));
                                let mut e =
                                    VDFSCatalogEntry::new(entry.file_name().to_str().unwrap());

                                e.next_index = offset + last_dir_index as u32 + 1;
                                e.typ |= EntryType::Dir as u32;
                                if entry_index == paths.len() - 1 {
                                    // last_dir_index {
                                    e.typ |= EntryType::LastFile as u32;
                                }
                                self.catalog.push(e);
                                offset += 1;
                            } else {
                                let mut e = VDFSCatalogEntry::new_sized(
                                    entry.file_name().to_str().unwrap(),
                                    entry.metadata().unwrap().len(),
                                );
                                e.next_index = level; //offset ;
                                if entry_index == paths.len() - 1 {
                                    e.typ = EntryType::LastFile as u32;
                                }
                                self.files.push(e);
                                match fs::read(&path) {
                                    Ok(mut d) => self.data.append(&mut d),
                                    Err(e) => {
                                        eprintln!("ERROR: {}", e);
                                        exit(69);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error reading directory: {}", err);
                }
            }
        }

        let final_num = self.catalog.len() + self.files.len();
        self.header.catalog_offset = 296 as u32;
        self.header.num_files = final_num as u32;
        self.header.num_entries = self.files.len() as u32;

        let mut last_offset = 0;
        for (i, f) in self.files.iter_mut().enumerate() {
            if last_offset != f.next_index {
                last_offset = f.next_index;
                if f.next_index != 0 {
                    let num_entry = self.catalog.len() as u32 + i as u32;
                    if let Some(o) = self.catalog.get_mut((f.next_index) as usize) {
                        o.next_index = num_entry;
                    }
                }
            }
            //                               length of the file entries
            f.next_index =
                self.header.catalog_offset + self.header.num_files * 80 + self.curr_pos as u32;
            self.curr_pos += f.size;
        }
    }

    fn calculate_data_size(&mut self) {
        self.header.size = self.files.iter().map(|entry| entry.size).sum();
    }

    pub fn comment(&mut self, cmnt: &str) {
        self.header.comment(cmnt);
    }
}
