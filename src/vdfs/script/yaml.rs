use super::VdfsScript;
use anyhow::Result;

impl<'a> VdfsScript<'a> {
    pub fn from_yaml(yml_file: &'a str) -> Result<Self> {
        let vdf: VdfsScript = serde_yaml::from_str(yml_file)?;
        Ok(vdf)
    }
}
