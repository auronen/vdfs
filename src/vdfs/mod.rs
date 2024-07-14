use anyhow::Result;
use chrono::{Datelike, Timelike};
use core::fmt;
use glob::{glob_with, MatchOptions};
use memmap2::Mmap;
use ptree::{print_tree_with, PrintConfig};
use std::{
    collections::VecDeque,
    fs::{self, read_to_string, File},
    io::{BufWriter, Write},
    path::PathBuf,
    process::exit,
    time::Instant,
};

mod filetree;
mod parser;
pub mod script;

use crate::vdfs::{filetree::build_file_system_tree_filtered, script::VdfsScript};

use self::{
    filetree::{build_file_system_tree, FileSystemNode},
    parser::parse_vdfs,
};

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
        let len = cmnt.len();
        if len > 256 {
            self.comment[..256].copy_from_slice(cmnt[0..256].as_bytes());
        } else {
            self.comment[..cmnt.len()].copy_from_slice(cmnt.as_bytes());
        }
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
    time |= (curr.month0() + 1) << 21;
    time |= curr.day() << 16;
    time |= curr.hour() << 11;
    time |= curr.minute() << 5;
    time |= curr.second() / 2;

    time
}

enum EntryType {
    Dir = 0x80000000,
    LastFile = 0x40000000,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct VDFSCatalogEntry {
    name_utf8: String,
    name: [u8; 64],
    offset: u32,
    size: u32,
    typ: u32,
    attributes: u32,

    parent_id: i32,
    // is_dir: bool,
}
impl VDFSCatalogEntry {
    fn new(file_name: &str) -> VDFSCatalogEntry {
        let mut vdfs = VDFSCatalogEntry::default();
        vdfs.name[..file_name.len()].copy_from_slice(file_name.to_ascii_uppercase().as_bytes());
        vdfs.name_utf8 = file_name.to_string();
        vdfs
    }
    fn new_sized(file_name: &str, size: u64) -> VDFSCatalogEntry {
        let mut vdfs = VDFSCatalogEntry::default();
        vdfs.name[..file_name.len()].copy_from_slice(file_name.to_ascii_uppercase().as_bytes());
        vdfs.name_utf8 = file_name.to_string();
        vdfs.size = size as u32;
        vdfs
    }

    fn is_dir(&self) -> bool {
        (self.typ & EntryType::Dir as u32) != 0
    }

    fn is_last(&self) -> bool {
        (self.typ & EntryType::LastFile as u32) != 0
    }
}

impl fmt::Display for VDFSCatalogEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = String::from_utf8_lossy(&self.name);

        writeln!(f, "Name: {}", name)?;
        writeln!(f, "Offset: {}", self.offset)?;
        writeln!(f, "Size: {}", self.size)?;

        writeln!(f, "par_id: {}", self.parent_id)?;

        writeln!(f, "Type: {}", self.typ)?;

        Ok(())
    }
}

impl Default for VDFSCatalogEntry {
    fn default() -> Self {
        VDFSCatalogEntry {
            name_utf8: String::new(),
            name: [0x20; 64],
            offset: 0,
            size: 0,
            typ: 0,
            attributes: 0,

            parent_id: 0,
            // is_dir: false,
        }
    }
}

#[derive(Debug)]
pub struct Vdfs {
    pub header: VDFSHeader,
    pub fs: FileSystemNode,

    pub catalog_dirs: Vec<VDFSCatalogEntry>,
    pub data: Vec<u8>,
    pub curr_pos: u32,
}

impl fmt::Display for Vdfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "VDFS Header:")?;
        writeln!(f, "{}", self.header)?;

        writeln!(f, "VDFS Catalog:")?;
        for (i, entry) in self.catalog_dirs.iter().enumerate() {
            writeln!(f, "{i}\n{}\n", entry)?;
        }

        Ok(())
    }
}

impl Vdfs {
    pub fn from_dir(path: &mut PathBuf) -> Self {
        let mut vdfs = Vdfs {
            header: VDFSHeader::default(),
            fs: build_file_system_tree(path, -1),
            catalog_dirs: Vec::new(),
            data: Vec::new(),
            curr_pos: 0,
        };

        vdfs.build_catalog();
        // bfs(&vdfs.fs);
        vdfs.calculate_data_size();
        vdfs
    }

    pub fn from_script(
        path: &PathBuf,
        base_dir_override: &Option<PathBuf>,
        output_file_override: &Option<PathBuf>,
        comment_override: &Option<String>,
    ) -> Result<()> {
        let time = Instant::now();
        println!("[INFO] Generating archive: {}", path.display());
        let yml_file = read_to_string(path).unwrap();
        let script = VdfsScript::from_yaml(&yml_file).unwrap();

        // println!("{:#?}", script);

        if script.base_dir.as_os_str().is_empty() && base_dir_override.is_none() {
            println!(
                "[ERROR] Empty base directory path in script file and no override was provided."
            );
            exit(1)
        } else if script.file_path.as_os_str().is_empty() && output_file_override.is_none() {
            println!("[ERROR] Empty output path in script file and no override was provided.");
            exit(1)
        }

        let path_filter_globs: Vec<_> = script
            .file_include_globs
            .iter()
            .flat_map(|g| {
                let glb = format!(
                    "{}/{}",
                    case_insensitive_globify(&match base_dir_override {
                        Some(pb) => pb.to_string_lossy(),
                        None => script.base_dir.to_string_lossy(),
                    }),
                    case_insensitive_globify(g)
                );
                // println!("glob: {}", glb);
                glob_with(
                    &glb,
                    MatchOptions {
                        case_sensitive: false,
                        require_literal_separator: false,
                        require_literal_leading_dot: false,
                    },
                )
            })
            .collect();

        let mut path_filter: Vec<Vec<String>> = Vec::new();
        // println!("{:#?}", path_filter);

        for paths in path_filter_globs {
            for p in paths {
                if let Ok(path) = p {
                    path_filter.push({
                        let pth = path
                            .strip_prefix(match base_dir_override {
                                Some(pb) => {
                                    // let mut pb = pb.clone();
                                    // pb.pop();
                                    pb
                                }
                                None => {
                                    // let mut bd = script.base_dir.clone();
                                    // bd.pop();
                                    // bd
                                    &script.base_dir
                                }
                            })
                            .unwrap();
                        pth.iter()
                            .map(|component| component.to_string_lossy().to_string())
                            .collect()
                    });
                }
            }
        }

        let mut vdfs = Vdfs {
            header: VDFSHeader::default(),
            fs: build_file_system_tree_filtered(
                match base_dir_override {
                    Some(pb) => pb,
                    None => &script.base_dir,
                },
                -1,
                &path_filter,
            ),
            catalog_dirs: Vec::new(),
            data: Vec::new(),
            curr_pos: 0,
        };
        // println!("-------");
        // bfs(&vdfs.fs);
        // println!("-------");
        // println!("{:#?}", path_filter);

        vdfs.build_catalog();

        // bfs(&vdfs.fs);
        // println!("{}", vdfs);
        vdfs.calculate_data_size();
        println!("[INFO] Done: {:.2?}", time.elapsed());
        vdfs.add_comment(match comment_override {
            Some(s) => Some(s),
            None => Some(script.comment),
        })
        .save_to_file(match output_file_override {
            Some(o) => o,
            None => &script.file_path,
        })?;
        Ok(())
    }

    fn build_catalog(&mut self) {
        let mut queue = VecDeque::new();
        queue.push_back((-1, &self.fs));

        let mut index = -1;
        while !queue.is_empty() {
            let (par, node) = queue.pop_front().unwrap();

            match node {
                FileSystemNode::Directory {
                    name,
                    path: _,
                    is_last,
                    children,
                    level: _,
                } => {
                    if node != &self.fs {
                        let mut e = VDFSCatalogEntry::new(name);
                        // e.is_dir = true;
                        e.typ |= EntryType::Dir as u32;
                        if *is_last {
                            e.typ |= EntryType::LastFile as u32;
                        }
                        e.parent_id = par;

                        self.catalog_dirs.push(e);
                        for child in children {
                            queue.push_back((index, child));
                        }
                    } else {
                        for child in children {
                            queue.push_back((index, child));
                        }
                    }
                }
                FileSystemNode::File {
                    name,
                    path,
                    data_offset: _,
                    data_size: _,
                    is_last,
                    level: _,
                } => {
                    let mut e = VDFSCatalogEntry::new_sized(
                        name,
                        match fs::metadata(path) {
                            Ok(m) => m.len(),
                            Err(e) => {
                                eprintln!("ERROR: {}", e);
                                exit(420);
                            }
                        },
                    );
                    // e.is_dir = false;
                    e.parent_id = par;

                    if *is_last {
                        e.typ = EntryType::LastFile as u32;
                    }
                    self.catalog_dirs.push(e);
                    match fs::read(path) {
                        Ok(mut d) => self.data.append(&mut d),
                        Err(e) => {
                            eprintln!("ERROR: {}", e);
                            exit(69);
                        }
                    }
                }
            }
            index += 1;
        }

        let mut queue = VecDeque::new();
        queue.push_back(&self.fs);

        let mut i = -1;
        while !queue.is_empty() {
            let node = queue.pop_front().unwrap();

            match node {
                FileSystemNode::Directory {
                    name: _,
                    path: _,
                    children,
                    is_last: _,
                    level: _,
                } => {
                    if node != &self.fs {
                        let _id = self.find_index(i as u32);
                        self.catalog_dirs[i as usize].offset = _id;

                        for child in children {
                            queue.push_back(child);
                        }
                    } else {
                        for child in children {
                            queue.push_back(child);
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }

        let final_num = self.catalog_dirs.len(); // + self.catalog_files.len();
        self.header.catalog_offset = 296_u32;
        self.header.num_files = final_num as u32;
        self.header.num_entries = self
            .catalog_dirs
            .iter()
            .filter(|f| f.typ == 0 || f.typ == EntryType::LastFile as u32)
            .count() as u32; // self.catalog_files.len() as u32;

        self.catalog_dirs
            .iter_mut()
            .filter(|f| f.typ == 0 || f.typ == EntryType::LastFile as u32)
            .for_each(|f| {
                f.offset = self.header.catalog_offset + self.header.num_files * 80 + self.curr_pos;
                self.curr_pos += f.size;
            });
    }

    fn find_index(&self, level: u32) -> u32 {
        match self
            .catalog_dirs
            .iter()
            .position(|item| item.parent_id as u32 == level)
        {
            Some(i) => i as u32,
            None => 0_u32,
        }
    }

    // This could be done elegantly with serde, but I don't know how to use it :kekw:
    pub fn save_to_file(&self, output_file: &PathBuf) -> Result<(), std::io::Error> {
        let time = Instant::now();
        println!("[INFO] Writing {}", output_file.display());
        let file = File::create(output_file)?;

        let mut buf_writer = BufWriter::new(file);

        buf_writer.write_all(&self.header.comment)?;
        buf_writer.write_all(&self.header.signature)?;
        buf_writer.write_all(&self.header.num_files.to_le_bytes())?;
        buf_writer.write_all(&self.header.num_entries.to_le_bytes())?;
        buf_writer.write_all(&self.header.timestamp.to_le_bytes())?;
        buf_writer.write_all(&self.header.size.to_le_bytes())?;
        buf_writer.write_all(&self.header.catalog_offset.to_le_bytes())?;
        buf_writer.write_all(&self.header.version.to_le_bytes())?;

        for c in &self.catalog_dirs {
            buf_writer.write_all(&c.name)?;
            buf_writer.write_all(&c.offset.to_le_bytes())?;
            buf_writer.write_all(&c.size.to_le_bytes())?;
            buf_writer.write_all(&c.typ.to_le_bytes())?;
            buf_writer.write_all(&c.attributes.to_le_bytes())?;
        }

        buf_writer.write_all(&self.data)?;

        buf_writer.flush()?;
        println!("[INFO] Done: {:.2?}", time.elapsed());
        Ok(())
    }

    fn calculate_data_size(&mut self) {
        self.header.size = self.catalog_dirs.iter().map(|entry| entry.size).sum();
    }

    pub fn add_comment(mut self, cmnt: Option<&str>) -> Self {
        self.header.comment(match cmnt {
            Some(c) => c,
            None => "",
        });
        self
    }

    // pub fn set_comment(&mut self, cmnt: &str) {
    //     self.header.comment(cmnt);
    // }

    pub fn from_mmap(map: &Mmap, file_name: &str) -> Self {
        // let f = fs::read(path).expect("the read to be succesful");
        // let file = File::open(path).expect("file to be valid");
        // let file_map = unsafe { Mmap::map(&file).unwrap()  };

        let vdfs = parse_vdfs(
            &map,
            file_name, // path.file_name()
                      //     .expect("file name to be valid")
                      //     .to_str()
                      //     .expect("to be able to convert into str"),
        )
        .expect("to work");
        vdfs
    }

    pub fn print_tree(&self, depth: Option<u32>) {
        let mut conf = PrintConfig::default();
        if let Some(depth) = depth {
            conf.depth = depth;
        };
        let _ = print_tree_with(&self.fs, &conf);
        // println!("{:?}", x)
    }

    pub fn extract_file(&self, file_name: &str, mmap: &Mmap) -> () {
        if let Some(node) = self.fs.find_node_by_name(file_name) {
            if let FileSystemNode::File {
                name,
                data_offset,
                data_size,
                ..
            } = node
            {
                let mut file = File::create(PathBuf::from(name)).unwrap();
                if let Some(offset) = data_offset {
                    file.write_all(&mmap[*offset as usize..*offset as usize + data_size])
                        .unwrap();
                } else {
                    eprintln!("This should be also unreachable...");
                    exit(1);
                }
            } else {
                unreachable!("This cannot happen");
            }
        } else {
            eprintln!("[ERROR] Could not find file `{file_name}`");
            exit(1);
        }
    }
}

fn is_on_level(filters: &Vec<Vec<String>>, search_term: &str, level: i32) -> bool {
    if level == -1 {
        return true;
    }
    for filter in filters {
        if filter.len() > level as usize && filter[level as usize].eq_ignore_ascii_case(search_term)
        {
            return true;
        }
    }
    return false;
}

fn case_insensitive_globify(input: &str) -> String {
    let mut s = String::new();
    for c in input.chars() {
        if c.is_alphabetic() {
            s.push('[');
            s.push(c.to_ascii_lowercase());
            s.push(c.to_ascii_uppercase());
            s.push(']');
        } else {
            s.push(c);
        }
    }
    s
}
