use std::{path::PathBuf, process::exit, io, borrow::Cow};

use ptree::{TreeItem, Style};

use super::is_on_level;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileSystemNode {
    Directory {
        name: String,
        path: PathBuf,
        children: Vec<FileSystemNode>,

        level: i32,
        is_last: bool,
    },
    File {
        name: String,
        path: PathBuf,

        is_last: bool,
        level: i32,
    },
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
            FileSystemNode::File { .. } =>  Cow::from(vec![]),
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
                is_last: false,
                level: lvl,
            })
        } else {
            None
        }
    } else {
        let dir_name = path.file_name().unwrap().to_string_lossy().into_owned();
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
        println!("[ERROR] You cannot add a single dile like that!");
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

// pub fn bfs(root: &FileSystemNode) {
//     use std::collections::VecDeque;
//     let mut queue = VecDeque::new();
//     queue.push_back(root);

//     while !queue.is_empty() {
//         let node = queue.pop_front().unwrap();

//         match node {
//             FileSystemNode::Directory {
//                 name,
//                 children,
//                 level,
//                 ..
//             } => {
//                 println!("{:>15}\t({level}) g", name);
//                 for child in children {
//                     queue.push_back(child);
//                 }
//             }
//             FileSystemNode::File { name, level, .. } => {
//                 println!("{:>15}\t({level}) f", name);
//             }
//         }
//     }
// }
