use std::path::PathBuf;


#[derive(Debug)]
pub enum FileSystemNode {
    Directory {
        name: String,
        path: PathBuf,
        children: Vec<FileSystemNode>,
    },
    File {
        name: String,
        path: PathBuf,
    },
}
