use std::path::PathBuf;

use anyhow::{anyhow, Result};
use nom::{bytes::complete::take, multi::count, number::complete::le_u32, IResult, Finish};
use yore::code_pages::CP1252;

use super::{VDFSCatalogEntry, VDFSHeader, Vdfs};

fn parse_vdfs(s: &[u8]) -> Result<Vdfs> {
    // parse_vdfs_(s).finish().map(|r| r.1)
    match parse_vdfs_(s).finish() {
        Ok(r) => Ok(r.1),
        Err(_) => Err(anyhow!("error while parsing vdfs")),
    }

}

fn parse_vdfs_(s: &[u8]) -> IResult<&[u8], Vdfs> {
    let (s, header) = parse_header(s)?;
    let (s, catalog) = count(parse_entry, header.num_entries as usize)(s)?;
    Ok((s, Vdfs {
        header,
        fs: super::filetree::FileSystemNode::Directory {
            name: "".to_string(),
            path: PathBuf::from(""),
            children: vec![],
            level: -1,
            is_last: true,
        },
        catalog_dirs: catalog,
        data: s.to_vec(),
        curr_pos: 0,
    }))
}

fn parse_header(s: &[u8]) -> IResult<&[u8], VDFSHeader> {
    let (s, comment) = take(256usize)(s)?;
    let (s, signature) = take(16usize)(s)?;
    let (s, num_files) = le_u32(s)?;
    let (s, num_entries) = le_u32(s)?;
    let (s, timestamp) = le_u32(s)?;
    let (s, size) = le_u32(s)?;
    let (s, catalog_offset) = le_u32(s)?;
    let (s, version) = le_u32(s)?;
    Ok((
        s,
        VDFSHeader {
            comment: comment.try_into().expect("nom to not break here :jjp:"),
            signature: signature.try_into().expect("nom to not break here :jjp:"),
            num_files,
            num_entries,
            timestamp,
            size,
            catalog_offset,
            version,
        },
    ))
}

fn parse_entry(s: &[u8]) -> IResult<&[u8], VDFSCatalogEntry> {
    let (s, name) = take(64usize)(s)?;
    let (s, next_index) = le_u32(s)?;
    let (s, size) = le_u32(s)?;
    let (s, typ) = le_u32(s)?;
    let (s, attributes) = le_u32(s)?;
    Ok((
        s,
        VDFSCatalogEntry {
            name_utf8: CP1252.decode(name).to_string(),
            name: name.try_into().expect("nom to be successful here"),
            next_index,
            size,
            typ,
            attributes,
            parent_id: -1,
        },
    ))
}
