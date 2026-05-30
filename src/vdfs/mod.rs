use anyhow::{Context, Result};
use chrono::{Datelike, Timelike};
use core::fmt;
use glob::{glob_with, MatchOptions};
use indicatif::ProgressBar;
use memmap2::Mmap;
use std::{
    collections::BTreeMap,
    fs::{self, read_to_string, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::exit,
    time::Instant,
};
use tree_ds::prelude::TraversalStrategy;

mod filetree;
mod parser;
pub mod script;

use crate::vdfs::script::VdfsScript;

use self::{
    filetree::{FSNode, FileSystemTree, PathTree},
    parser::parse_vdfs,
};

#[allow(dead_code)]
#[derive(Debug)]
pub struct VDFSHeader {
    comment: [u8; 256],
    signature: [u8; 16],
    num_entries: u32,
    num_files: u32,
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
        writeln!(f, "Number of Files: {}", self.num_entries)?;
        writeln!(f, "Number of Entries: {}", self.num_files)?;
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
            num_entries: 0,
            num_files: 0,
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
    // pub fs: FileSystemNode,
    pub fs: FileSystemTree,

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
            // fs: build_file_system_tree(path, -1),
            fs: FileSystemTree::build_fs_tree(path),
            catalog_dirs: Vec::new(),
            data: Vec::new(),
            curr_pos: 0,
        };

        vdfs.build_catalog2();
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

        if script.base_dir.as_os_str().is_empty() && base_dir_override.is_none() {
            println!(
                "[ERROR] Empty base directory path in script file and no override was provided."
            );
            exit(1)
        } else if script.file_path.as_os_str().is_empty() && output_file_override.is_none() {
            println!("[ERROR] Empty output path in script file and no override was provided.");
            exit(1)
        }

        let paths: Vec<_> = script
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

                let pths = glob_with(
                    &glb,
                    MatchOptions {
                        case_sensitive: false,
                        require_literal_separator: false,
                        require_literal_leading_dot: false,
                    },
                )
                .expect("globs to not fail");

                let paths: Vec<_> = pths.filter_map(Result::ok).collect();
                paths
            })
            .collect();

        let mut vdfs = Vdfs {
            header: VDFSHeader::default(),
            fs: FileSystemTree::build_file_system_tree_filtered(
                match base_dir_override {
                    Some(pb) => pb,
                    None => &script.base_dir,
                },
                &paths,
            ),
            catalog_dirs: Vec::new(),
            data: Vec::new(),
            curr_pos: 0,
        };

        vdfs.build_catalog2();

        vdfs.calculate_data_size();
        println!("[INFO] Done generating archive: {:.2?}", time.elapsed());
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

    fn build_catalog2(&mut self) {
        let id_cata = &self.build_index_catalog();

        let pb = ProgressBar::new(self.fs.0.num_of_files() as u64);

        let mut ids: Vec<(u128, bool)> = vec![(0, false); id_cata.len()];
        for (node_id, (id, is_last)) in id_cata {
            ids[*id] = (*node_id, *is_last);
        }

        for (node_id, last) in ids.iter().skip(1) {
            let node = self
                .fs
                .0
                .get_node_by_id(&node_id)
                .unwrap()
                .get_value()
                .unwrap();

            match node {
                FSNode::Directory { name, .. } => {
                    let mut e = VDFSCatalogEntry::new(&name);
                    e.typ |= EntryType::Dir as u32;
                    if *last {
                        e.typ |= EntryType::LastFile as u32;
                    }

                    let first_child = self
                        .fs
                        .0
                        .get_node_by_id(&node_id)
                        .unwrap()
                        .get_children_ids()
                        .first()
                        .expect("directory to have at least one child")
                        .clone();

                    e.offset = id_cata
                        .get(&first_child)
                        .expect("the node to be in the btree map")
                        .0 as u32
                        - 1;

                    self.catalog_dirs.push(e);
                }
                FSNode::File { name, path, .. } => {
                    let mut e = VDFSCatalogEntry::new_sized(
                        &name,
                        match fs::metadata(&path) {
                            Ok(m) => m.len(),
                            Err(e) => {
                                eprintln!("ERROR: {} ({})", e, path.display());
                                exit(420);
                            }
                        },
                    );

                    if *last {
                        e.typ = EntryType::LastFile as u32;
                    }
                    self.catalog_dirs.push(e);
                    match fs::read(path.clone()) {
                        Ok(mut d) => {
                            pb.inc(1);
                            self.data.append(&mut d)
                        }
                        Err(e) => {
                            eprintln!("ERROR: {} ({})", e, path.display());
                            exit(69);
                        }
                    }
                }
            }
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
        pb.finish_with_message("done");
    }

    fn build_index_catalog(&mut self) -> BTreeMap<u128, (usize, bool)> {
        let mut idxs: BTreeMap<u128, (usize, bool)> = BTreeMap::new();
        _ = self.recurse(
            &mut idxs,
            &self
                .fs
                .0
                .get_root_node()
                .expect("root node should exist")
                .get_node_id(),
        );
        idxs
    }

    fn recurse(&self, indx: &mut BTreeMap<u128, (usize, bool)>, node_id: &u128) {
        let children = self
            .fs
            .0
            .get_node_by_id(node_id)
            .unwrap()
            .get_children_ids();
        for (i, c) in children.iter().enumerate() {
            let id = indx.len();
            indx.insert(*c, (id, if i == children.len() - 1 { true } else { false }));
        }
        for c in &children {
            self.recurse(indx, c);
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
        println!("[INFO] Done writing: {:.2?}", time.elapsed());
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
        parse_vdfs(&map, file_name).expect("to work")
    }

    pub fn print_paths(&self, depth: Option<u32>) {
        // prints paths as full paths inside the vdfs archive
        // limited by maximum depth
        let root_id = self.fs.0.get_root_node().unwrap().get_node_id();

        for node_id in self
            .fs
            .0
            .traverse(&root_id, TraversalStrategy::PreOrder)
            .unwrap()
        {
            if node_id == root_id {
                continue;
            }

            let node = self.fs.0.get_node_by_id(&node_id).unwrap();
            let fs_node = node.get_value().unwrap();

            // Check depth limit
            if let Some(max_depth) = depth {
                let current_depth = self.fs.0.get_node_depth(&node_id).unwrap();
                if current_depth as u32 > max_depth {
                    continue;
                }
            }

            let FSNode::File { ref path, .. } = fs_node else {
                continue;
            };

            println!("{}", path.display());
        }
    }

    pub fn print_tree(&self, _depth: Option<u32>) {
        println!("{}", self.fs.0);
    }

    pub fn extract_file(&self, file_name: &str, mmap: &Mmap) -> () {
        for node_id in self
            .fs
            .0
            .traverse(
                &self.fs.0.get_root_node().unwrap().get_node_id(),
                TraversalStrategy::InOrder,
            )
            .unwrap()
            .iter()
        {
            if let FSNode::File {
                name,
                data_offset,
                data_size,
                ..
            } = self
                .fs
                .0
                .get_node_by_id(node_id)
                .unwrap()
                .get_value()
                .unwrap()
            {
                let mut file = File::create(PathBuf::from(name)).unwrap();
                if let Some(offset) = data_offset {
                    file.write_all(&mmap[offset as usize..offset as usize + data_size])
                        .unwrap();
                } else {
                    eprintln!("This should be also unreachable...");
                    exit(1);
                }
            }
        }
        eprintln!("[ERROR] Could not find file `{file_name}`");
        exit(1);
    }

    pub fn extract_all(&self, target_dir: &Path, mmap: &Mmap) -> Result<()> {
        let start_time = Instant::now();

        // Ensure the base target directory exists.
        fs::create_dir_all(target_dir).with_context(|| {
            format!(
                "Failed to create target directory '{}'",
                target_dir.display()
            )
        })?;

        let root_id = self
            .fs
            .0
            .get_root_node()
            .context("VDFS tree has no root node")?
            .get_node_id();

        // Pre-order traversal so directories are created before files within them.
        for node_id in self.fs.0.traverse(&root_id, TraversalStrategy::PreOrder)? {
            if node_id == root_id {
                continue;
            }

            let node = self.fs.0.get_node_by_id(&node_id).with_context(|| {
                format!("Internal error: Failed to get node with ID {}", node_id)
            })?;
            let fs_node = node.get_value().with_context(|| {
                format!("Internal error: Node with ID {} has no value", node_id)
            })?;

            let relative_path = match fs_node {
                FSNode::Directory { ref path, .. } => path,
                FSNode::File { ref path, .. } => path,
            };

            let full_disk_path = target_dir.join(relative_path);

            match fs_node {
                FSNode::Directory { .. } => {
                    fs::create_dir_all(&full_disk_path).with_context(|| {
                        format!("Failed to create directory '{}'", full_disk_path.display())
                    })?;
                }
                FSNode::File {
                    data_offset,
                    data_size,
                    ..
                } => {
                    // Create the file to write into.
                    let mut file = File::create(&full_disk_path).with_context(|| {
                        format!("Failed to create file '{}'", full_disk_path.display())
                    })?;

                    if let Some(offset) = data_offset {
                        // Here we directly index into the memory map as in extract_file.
                        file.write_all(&mmap[offset as usize..offset as usize + data_size])
                            .with_context(|| {
                                format!(
                                    "Failed to write data to file '{}'",
                                    full_disk_path.display()
                                )
                            })?;
                    } else {
                        // If there's no data offset but a non-zero size, log a warning.
                        if data_size != 0 {
                            eprintln!(
                            "[WARN] File '{}' has size {} but no data offset in VDFS tree. Creating empty file.",
                            relative_path.display(),
                            data_size
                        );
                        }
                    }
                }
            }
        }

        println!("[INFO] Finished extraction in {:.2?}", start_time.elapsed());
        Ok(())
    }
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
