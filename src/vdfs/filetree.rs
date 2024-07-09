use std::{borrow::Cow, io, path::PathBuf, process::exit};

use ptree::{Style, TreeItem};
use walkdir::WalkDir;

use super::{is_on_level, VDFSCatalogEntry};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileSystemNode {
    Directory {
        name: String,
        path: PathBuf,
        children: Vec<FileSystemNode>,

        // for building the tree from directory
        level: i32,
        is_last: bool,
    },
    File {
        name: String,
        path: PathBuf,

        // for reading
        data_offset: Option<u32>,
        data_size: usize,

        // for building the tree from directory
        level: i32,
        is_last: bool,
    },
}

impl FileSystemNode {
    pub fn new_from(entries: &Vec<VDFSCatalogEntry>, name: &str) -> Self {
        FileSystemNode::Directory {
            name: name.to_string(),
            path: PathBuf::default(),
            children: FileSystemNode::generate_children(entries, 0),
            level: -1,
            is_last: false,
        }
    }

    fn generate_children(
        entries: &[VDFSCatalogEntry],
        starting_index: usize,
    ) -> Vec<FileSystemNode> {
        let mut children = Vec::new();
        for e in entries.iter().skip(starting_index) {
            if e.is_dir() {
                children.push(FileSystemNode::Directory {
                    name: e.name_utf8.clone(),
                    path: PathBuf::default(), // TODO: build a path here???
                    children: FileSystemNode::generate_children(entries, e.next_index as usize),
                    level: -1,
                    is_last: e.is_last(),
                });
                if e.is_last() {
                    return children;
                }
            } else {
                children.push(FileSystemNode::File {
                    name: e.name_utf8.clone(),
                    path: PathBuf::default(), // TODO: build a path here???
                    data_offset: Some(e.next_index),
                    data_size: e.size as usize,
                    is_last: e.is_last(),
                    level: -1,
                });
                if e.is_last() {
                    return children;
                }
            }
        }
        children
    }

    pub fn find_node_by_name(&self, name: &str) -> Option<&FileSystemNode> {
        match self {
            FileSystemNode::File {
                name: node_name, ..
            } if node_name == name => Some(self),
            FileSystemNode::Directory { children, .. } => {
                for child in children {
                    if let Some(found) = child.find_node_by_name(name) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

trait Name {
    fn name(&self) -> &str;
}

impl Name for FileSystemNode {
    fn name(&self) -> &str {
        match self {
            FileSystemNode::Directory { name, .. } => name,
            FileSystemNode::File { name, .. } => name,
        }
    }
}

impl TreeItem for FileSystemNode {
    type Child = Self;
    fn write_self<W: io::Write>(&self, f: &mut W, style: &Style) -> io::Result<()> {
        write!(f, "{}", style.paint(self.name()))
    }
    fn children(&self) -> Cow<[Self::Child]> {
        match self {
            FileSystemNode::Directory { children, .. } => Cow::from(children),
            FileSystemNode::File { .. } => Cow::from(vec![]),
        }
    }
}

impl FileSystemNode {
    fn cmp_file_system_nodes(a: &FileSystemNode, b: &FileSystemNode) -> std::cmp::Ordering {
        match (a, b) {
            (FileSystemNode::Directory { .. }, FileSystemNode::File { .. }) => {
                std::cmp::Ordering::Less
            }
            (FileSystemNode::File { .. }, FileSystemNode::Directory { .. }) => {
                std::cmp::Ordering::Greater
            }
            (
                FileSystemNode::Directory { name: name_a, .. },
                FileSystemNode::Directory { name: name_b, .. },
            )
            | (
                FileSystemNode::File { name: name_a, .. },
                FileSystemNode::File { name: name_b, .. },
            ) => name_a.to_uppercase().cmp(&name_b.to_uppercase()),
        }
    }
}

pub fn build_file_system_tree(path: &PathBuf, lvl: i32) -> FileSystemNode {
    if path.is_file() {
        return FileSystemNode::File {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path: path.to_path_buf(),
            data_offset: None,
            data_size: 0,
            is_last: false,
            level: lvl,
        };
    } else {
        let dir_name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut children = Vec::new();

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let entry_path = entry.path();
                    children.push(build_file_system_tree(&entry_path, lvl + 1));
                }
            }
        }

        // Sort children before creating the Directory node
        children.sort_by(FileSystemNode::cmp_file_system_nodes);

        if let Some(last_node) = children.last_mut() {
            match last_node {
                FileSystemNode::Directory { is_last, .. }
                | FileSystemNode::File { is_last, .. } => {
                    *is_last = true;
                }
            }
        }

        // this is the return
        FileSystemNode::Directory {
            name: dir_name,
            path: path.to_path_buf(),
            children,
            is_last: false,
            level: lvl,
        }
    }
}

pub fn _build_file_system_tree_filtered(
    path: &PathBuf,
    lvl: i32,
    filter: &Vec<Vec<String>>,
) -> Option<FileSystemNode> {
    if path.is_file() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if is_on_level(filter, &name, lvl) {
            Some(FileSystemNode::File {
                name,
                path: path.to_path_buf(),
                data_offset: None,
                data_size: 0,
                is_last: false,
                level: lvl,
            })
        } else {
            None
        }
    } else {
        let dir_name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !WalkDir::new(path)
            .into_iter()
            .any(|entry| entry.unwrap().file_type().is_file())
        {
            return None;
        }
        if is_on_level(filter, &dir_name, lvl) {
            let mut children = Vec::new();

            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let entry_path = entry.path();
                        let ch = _build_file_system_tree_filtered(&entry_path, lvl + 1, filter);
                        if let Some(child) = ch {
                            children.push(child);
                        }
                    }
                }
            }

            // Sort children before creating the Directory node
            children.sort_by(FileSystemNode::cmp_file_system_nodes);

            if let Some(last_node) = children.last_mut() {
                match last_node {
                    FileSystemNode::Directory { is_last, .. }
                    | FileSystemNode::File { is_last, .. } => {
                        *is_last = true;
                    }
                }
            }

            // this is the return
            Some(FileSystemNode::Directory {
                name: dir_name,
                path: path.to_path_buf(),
                children,
                is_last: false,
                level: lvl,
            })
        } else {
            None
        }
    }
}

pub fn build_file_system_tree_filtered(
    path: &PathBuf,
    lvl: i32,
    filter: &Vec<Vec<String>>,
) -> FileSystemNode {
    if path.is_file() {
        println!("[ERROR] You cannot add a single file like that!");
        exit(1);
    } else {
        let dir_name = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut children = Vec::new();

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let entry_path = entry.path();
                    let ch = _build_file_system_tree_filtered(&entry_path, lvl + 1, filter);
                    if let Some(child) = ch {
                        children.push(child);
                    }
                }
            }
        }

        // Sort children before creating the Directory node
        children.sort_by(FileSystemNode::cmp_file_system_nodes);

        if let Some(last_node) = children.last_mut() {
            match last_node {
                FileSystemNode::Directory { is_last, .. }
                | FileSystemNode::File { is_last, .. } => {
                    *is_last = true;
                }
            }
        }

        // this is the return
        FileSystemNode::Directory {
            name: dir_name,
            path: path.to_path_buf(),
            children,
            is_last: false,
            level: lvl,
        }
    }
}
