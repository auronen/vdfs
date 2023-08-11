use std::path::PathBuf;

use serde::{Deserialize, Serialize};

mod vm;
mod yaml;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct VdfsScript<'a> {
    pub comment: &'a str,
    pub base_dir: PathBuf,
    pub file_path: PathBuf,
    pub file_include_globs: Vec<&'a str>,
    // pub file_exclude_globs: Vec<&'a str>,
}
