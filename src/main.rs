mod vdfs;

use anyhow::Result;
use std::{path::PathBuf, process::exit};

use clap::Parser;
use vdfs::Vdfs;

#[derive(Parser, Debug)]
#[command(term_width = 0, arg_required_else_help(true))]
struct Args {
    /// The base directory override
    #[arg(short = 'b', long, value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    base_directory: Option<std::path::PathBuf>,

    /// The output file override
    #[arg(short = 'o', long, value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
    output_file: Option<std::path::PathBuf>,

    /// Comment to be added to the volume
    #[arg(short = 'c', long)]
    comment: Option<String>,

    /// The yaml script or base directory
    #[arg()]
    input: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if !args.input.is_empty() {
        let mut path = PathBuf::from(args.input);
        if path.is_dir() {
            Vdfs::from_dir(&mut path)
                .add_comment(args.comment.as_deref())
                .save_to_file(&match args.output_file {
                    Some(p) => p,
                    None => {
                        path.push("DEFAULT.VDF");
                        path
                    }
                })?;
        } else if path.is_file() {
            Vdfs::from_script(
                &path,
                &args.base_directory,
                &args.output_file,
                &args.comment,
            )?;
        } else {
            eprintln!("This should not happen...");
            exit(1);
        }
    } else {
        eprintln!("Please provide a yaml file or a base directory.");
        exit(1);
    }

    Ok(())
}
