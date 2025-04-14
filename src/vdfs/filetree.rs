use core::fmt;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::exit,
};
use tree_ds::prelude::*;

use super::VDFSCatalogEntry;

#[derive(Debug, Default)]
pub struct FileSystemTree(pub Tree<AutomatedId, FSNode>);

pub trait PathTree {
    fn add_path(&mut self, path: &PathBuf, base_dir: &PathBuf);
    // fn do_component(&mut self, component: &OsStr, par: &u128) -> Option<u128>;
    fn recurse(
        &mut self,
        path_components: &mut Vec<&OsStr>,
        node_id: &u128,
        id: usize,
        path: &PathBuf,
    );
    fn add_components(
        &mut self,
        path_components: &mut Vec<&OsStr>,
        node_id: &u128,
        id: usize,
        path: &PathBuf,
    );
    fn num_of_files(&self) -> usize;
}

impl PathTree for Tree<AutomatedId, FSNode> {
    fn num_of_files(&self) -> usize {
        self.get_nodes()
            .iter()
            .filter(|&x| {
                if let Some(node) = x.get_value() {
                    node.is_file()
                } else {
                    false
                }
            })
            .count()
    }

    fn add_path(&mut self, path: &PathBuf, base_dir: &PathBuf) {
        let path_ = path
            .strip_prefix(base_dir)
            .expect("the prefix to be stripped")
            .to_path_buf();
        let mut path_components: Vec<&OsStr> = path_.components().map(|c| c.as_os_str()).collect();
        let head = self
            .get_root_node()
            .expect("the tree to have root")
            .get_node_id();
        self.recurse(&mut path_components, &head, 0, &path);
    }

    fn recurse(
        &mut self,
        path_components: &mut Vec<&OsStr>,
        node_id: &u128,
        id: usize,
        path: &PathBuf,
    ) {
        let name = path_components[id];
        let mut found = false;
        for ch in self
            .get_node_by_id(node_id)
            .expect("node to be valid")
            .get_children_ids()
        {
            let node = self.get_node_by_id(&ch).unwrap().get_value().unwrap();
            if name.eq_ignore_ascii_case(node.name()) {
                self.recurse(path_components, &ch, id + 1, path);
                found = true;
            }
        }
        if found == false {
            self.add_components(path_components, node_id, id, path);
        }
    }

    fn add_components(
        &mut self,
        path_components: &mut Vec<&OsStr>,
        parent_id: &u128,
        id: usize,
        path: &PathBuf,
    ) {
        // this is a directory
        if path_components.len() - 1 != id {
            let parent = self
                .add_node(
                    Node::new_with_auto_id(Some(FSNode::Directory {
                        name: path_components[id]
                            .to_ascii_uppercase()
                            .to_string_lossy()
                            .to_string(),
                        path: PathBuf::default(),
                        is_last: true,
                    })),
                    Some(parent_id),
                )
                .unwrap();
            self.add_components(path_components, &parent, id + 1, path);
            // this is a file
        } else {
            self.add_node(
                Node::new_with_auto_id(Some(FSNode::File {
                    name: path_components[id]
                        .to_ascii_uppercase()
                        .to_string_lossy()
                        .to_string(),
                    path: path.to_path_buf(),
                    data_offset: None,
                    data_size: 0,
                    is_last: false,
                })),
                Some(parent_id),
            )
            .unwrap();
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FSNode {
    Directory {
        name: String,
        path: PathBuf,
        is_last: bool,
    },
    File {
        name: String,
        path: PathBuf,
        data_offset: Option<u32>,
        data_size: usize,
        is_last: bool,
    },
}

impl FSNode {
    pub fn name(&self) -> &str {
        match self {
            FSNode::Directory { name, .. } => &name,
            FSNode::File { name, .. } => &name,
        }
    }
    pub fn is_file(&self) -> bool {
        match self {
            FSNode::Directory { .. } => false,
            FSNode::File { .. } => true,
        }
    }
}

impl FileSystemTree {
    pub fn new_from(entries: &Vec<VDFSCatalogEntry>, name: &str) -> Self {
        let mut tree = Tree::new(Some(name));
        let root = tree
            .add_node(
                Node::new_with_auto_id(Some(FSNode::Directory {
                    name: name.to_string(),
                    path: PathBuf::default(),
                    is_last: true,
                })),
                None,
            )
            .unwrap();
        FileSystemTree::generate_children(
            &mut tree,
            entries,
            0,
            Some(root),
            &PathBuf::default().as_path(),
        )
        .expect("the tree to be able to be constructed");
        FileSystemTree(tree)
    }

    fn generate_children(
        tree: &mut Tree<u128, FSNode>,
        entries: &[VDFSCatalogEntry],
        starting_index: u32,
        parent: Option<u128>,
        parent_path: &Path,
    ) -> Result<()> {
        for e in entries.iter().skip(starting_index as usize) {
            if e.is_dir() {
                let path = parent_path.to_path_buf().join(&e.name_utf8);
                let x = tree.add_node(
                    Node::new_with_auto_id(Some(FSNode::Directory {
                        name: e.name_utf8.clone(),
                        path: path.clone(), //: PathBuf::default(), // TODO: build a path here???
                        is_last: e.is_last(),
                    })),
                    parent.as_ref(),
                )?;

                FileSystemTree::generate_children(tree, entries, e.offset, Some(x), &path)?;

                if e.is_last() {
                    return Ok(());
                }
            } else {
                let path = parent_path.to_path_buf().join(&e.name_utf8);
                tree.add_node(
                    Node::new_with_auto_id(Some(FSNode::File {
                        name: e.name_utf8.clone(),
                        path, // : PathBuf::default(), // TODO: build a path here???
                        data_offset: Some(e.offset),
                        data_size: e.size as usize,
                        is_last: e.is_last(),
                    })),
                    parent.as_ref(),
                )?;

                if e.is_last() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    pub fn build_fs_tree(path: &PathBuf) -> Self {
        let mut tree = Tree::new(Some(
            &path
                .file_name()
                .unwrap()
                .to_str()
                .expect("the name to be valid"),
        ));
        let root = tree
            .add_node(
                Node::new_with_auto_id(Some(FSNode::Directory {
                    name: path.file_name().unwrap().to_string_lossy().to_string(),
                    path: PathBuf::default(),
                    is_last: true,
                })),
                None,
            )
            .unwrap();
        Self::build_file_system_tree(&mut tree, path, Some(root));
        FileSystemTree(tree.into())
    }

    pub fn build_file_system_tree(
        tree: &mut Tree<u128, FSNode>,
        path: &PathBuf,
        parent: Option<u128>,
    ) -> () {
        if path.is_file() {
            tree.add_node(
                Node::new_with_auto_id(Some(FSNode::File {
                    name: path.file_name().unwrap().to_string_lossy().into_owned(),
                    path: path.to_path_buf(),
                    data_offset: None,
                    data_size: 0,
                    is_last: false,
                })),
                parent.as_ref(),
            )
            .expect("");
        } else {
            if let Ok(entries) = std::fs::read_dir(path) {
                let par = tree
                    .add_node(
                        Node::new_with_auto_id(Some(FSNode::Directory {
                            name: path.file_name().unwrap().to_string_lossy().into_owned(),
                            path: path.to_path_buf(),
                            is_last: false,
                        })),
                        parent.as_ref(),
                    )
                    .expect("");
                for entry in entries {
                    if let Ok(entry) = entry {
                        let entry_path = entry.path();
                        Self::build_file_system_tree(tree, &entry_path, Some(par))
                    }
                }
            }
        }
    }

    pub fn build_file_system_tree_filtered(path_prefix: &PathBuf, paths: &Vec<PathBuf>) -> Self {
        if path_prefix.is_file() {
            eprintln!("[ERROR] You cannot add a single file like that!");
            exit(1);
        }
        let mut tree = Tree::new(Some(
            &path_prefix
                .file_name()
                .unwrap()
                .to_str()
                .expect("the name to be valid"),
        ));
        tree.add_node(
            Node::new_with_auto_id(Some(FSNode::Directory {
                name: path_prefix
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                path: PathBuf::default(),
                is_last: true,
            })),
            None,
        )
        .unwrap();
        for p in paths {
            tree.add_path(p, path_prefix);
        }
        FileSystemTree(tree.into())
    }
}

impl Default for FSNode {
    fn default() -> Self {
        FSNode::Directory {
            name: String::default(),
            path: PathBuf::default(),
            is_last: false,
        }
    }
}

impl fmt::Display for FSNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FSNode::Directory { name, is_last, .. } => {
                write!(f, "D: {}", name)?;
                if *is_last {
                    write!(f, " (last)")?;
                }
            }
            FSNode::File { name, is_last, .. } => {
                write!(f, "F: {}", name)?;
                if *is_last {
                    write!(f, " (last)")?;
                }
            }
        }
        Ok(())
    }
}
