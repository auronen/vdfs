use anyhow::{anyhow, Result};
use nom::{bytes::complete::take, multi::count, number::complete::le_u32, Finish, IResult};
use yore::code_pages::CP1252;

use super::{filetree::FileSystemTree, VDFSCatalogEntry, VDFSHeader, Vdfs};

pub fn parse_vdfs(s: &[u8], file_name: &str) -> Result<Vdfs> {
    match parse_vdfs_(s, file_name).finish() {
        Ok(r) => Ok(r.1),
        Err(_) => Err(anyhow!("error while parsing vdfs")),
    }
}

fn parse_vdfs_<'a>(s: &'a [u8], file_name: &'a str) -> IResult<&'a [u8], Vdfs> {
    let (s, header) = parse_header(s)?;
    let (s, catalog) = count(parse_entry, header.num_entries as usize)(s)?;
    Ok((
        s,
        Vdfs {
            header,
            fs: FileSystemTree::new_from(&catalog, file_name),
            catalog_dirs: catalog,
            data: s.to_vec(),
            curr_pos: 0,
        },
    ))
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
            num_entries: num_files,
            num_files: num_entries,
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
            name_utf8: CP1252.decode(name).trim().to_string(),
            name: name.try_into().expect("nom to be successful here"),
            offset: next_index,
            size,
            typ,
            attributes,
            parent_id: -1,
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::vdfs::filetree::FSNode;

    use super::parse_vdfs;

    #[test]
    fn parser_test() {
        // let path = "examples/Union.vdf";
        let path = "examples/test.vdf";
        // let path = "/home/auronen/.GOG/Gothic-1-classic-vanilla-patch/Data/textures_Startscreen_ohne_Logo.VDF";
        // let path = "/home/auronen/.GOG/Gothic-1-classic-vanilla-patch/Data/textures_choicebox_32pixel_modialpha.VDF";
        // let path = "examples/G1_cp1250.vdf";

        // let path = "examples/G1-deNotR/g2_cz1.vdf";
        // let path = "examples/G1-deNotR/g2_eng1.vdf";
        let path_buf = PathBuf::from(path);
        let f = fs::read(&path_buf).unwrap();

        let vdfs = parse_vdfs(
            &f,
            path_buf
                .file_name()
                .unwrap()
                .to_str()
                .expect("file name to be valid"),
        )
        .expect("to work");

        println!("num_entries: {}", vdfs.header.num_entries);
        println!("num_files: {}", vdfs.header.num_files);

        println!("catalog");
        for (i, c) in vdfs.catalog_dirs.iter().enumerate() {
            if c.is_dir() {
                println!(
                    "{:>2}: {} {} ({})",
                    i,
                    if c.is_dir() { ">" } else { " " },
                    c.name_utf8,
                    c.offset
                );
            } else {
                println!(
                    "{:>2}: {} {}",
                    i,
                    if c.is_dir() { ">" } else { " " },
                    c.name_utf8,
                );
            }
        }
        // println!("{}", &vdfs.fs.0);

        for n_id in vdfs
            .fs
            .0
            .traverse(
                &vdfs.fs.0.get_root_node().unwrap().get_node_id(),
                tree_ds::prelude::TraversalStrategy::PreOrder,
            )
            .unwrap()
            .iter()
        {
            let node = vdfs.fs.0.get_node_by_id(n_id).unwrap();
            match node.get_value().unwrap() {
                FSNode::Directory { name, .. } => println!(
                    "D: {} - {}",
                    name,
                    &vdfs.fs.0.get_node_depth(&node.get_node_id()).unwrap()
                ),
                FSNode::File { name, .. } => println!(
                    "F: {} - {}",
                    name,
                    &vdfs.fs.0.get_node_depth(&node.get_node_id()).unwrap()
                ),
            }
        }

        let mut x: Vec<_> = vdfs
            .fs
            .0
            .traverse(
                &vdfs.fs.0.get_root_node().unwrap().get_node_id(),
                tree_ds::prelude::TraversalStrategy::PreOrder,
            )
            .unwrap()
            .into_iter()
            .collect();

        x.sort_by(|a, b| {
            vdfs.fs
                .0
                .get_node_depth(a)
                .unwrap()
                .cmp(&vdfs.fs.0.get_node_depth(b).unwrap())
        });
        println!("");

        for n_id in x {
            let node = vdfs.fs.0.get_node_by_id(&n_id).unwrap();
            match node.get_value().unwrap() {
                FSNode::Directory { name, .. } => println!(
                    "D: {} - {}",
                    name,
                    &vdfs.fs.0.get_node_depth(&node.get_node_id()).unwrap()
                ),
                FSNode::File { name, .. } => println!(
                    "F: {} - {}",
                    name,
                    &vdfs.fs.0.get_node_depth(&node.get_node_id()).unwrap()
                ),
            }
        }

        assert_eq!(1, 2);
    }
}
