mod vdfs;

use anyhow::Result;
use memmap2::Mmap;
use std::{fs::File, path::PathBuf, process::exit};

use clap::{Parser, Subcommand};
use vdfs::Vdfs;

#[derive(Parser, Debug)]
#[command(term_width = 0, arg_required_else_help(true))]
struct Args {
    /// Mode of operation
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create new VDF or MOD archive
    #[command(arg_required_else_help(true))]
    Write {
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
    },

    /// Read VDF or MOD archives
    #[command(arg_required_else_help(true))]
    Read {
        /// VDF or MOD archive(s) to print out their contents tree
        #[arg(value_name = "FILE(s)", value_hint = clap::ValueHint::FilePath)]
        input: Vec<std::path::PathBuf>,

        /// Print paths
        #[arg(action, short = 'p', long)]
        paths: bool,

        /// Maximum depth of the tree view
        #[arg(short = 'L', long)]
        level: Option<u32>,
    },

    /// Extract VDF or MOD archives or individual files
    #[command(arg_required_else_help(true))]
    Extract {
        /// VDF or MOD archive(s) to extract
        #[arg(value_name = "FILE(s)", value_hint = clap::ValueHint::FilePath)]
        input: Vec<std::path::PathBuf>,

        /// File to extract
        #[arg(short = 'f', long)]
        file_name: Option<String>,

        /// Output directory
        #[arg(
            short = 'o',
            long,
            value_name = "DIR",
            default_value = ".",
            hide_default_value = true
        )]
        output_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Write {
            base_directory,
            output_file,
            comment,
            input,
        } => {
            if !input.is_empty() {
                let mut path = PathBuf::from(input);
                if path.is_dir() {
                    Vdfs::from_dir(&mut path)
                        .add_comment(comment.as_deref())
                        .save_to_file(&match output_file {
                            Some(p) => p,
                            None => {
                                path.push("DEFAULT.VDF");
                                path
                            }
                        })?;
                } else if path.is_file() {
                    Vdfs::from_script(&path, &base_directory, &output_file, &comment)?;
                } else {
                    eprintln!("This should not happen...");
                    exit(1);
                }
            } else {
                eprintln!("Please provide a yaml file or a base directory.");
                exit(1);
            }
        }
        Commands::Read {
            input,
            level,
            paths,
        } => input.iter().for_each(|path| {
            let file = File::open(path).expect("file to be valid");
            let file_map = unsafe { Mmap::map(&file).unwrap() };
            let vdfs = Vdfs::from_mmap(
                &file_map,
                path.file_name()
                    .expect("file name to be valid")
                    .to_str()
                    .expect("to be able to convert into str"),
            );
            if paths {
                vdfs.print_paths(level)
            } else {
                vdfs.print_tree(level);
            }
        }),
        Commands::Extract {
            input,
            file_name,
            output_dir,
        } => {
            if &input.len() > &1 {
                eprintln!("[ERROR] Specific file extraction works only with one archive provided");
            } else {
                let path = &input[0];

                let file = File::open(path).expect("file to be valid");
                let file_map = unsafe { Mmap::map(&file).unwrap() };

                let vdfs = Vdfs::from_mmap(
                    &file_map,
                    path.file_name()
                        .expect("file name to be valid")
                        .to_str()
                        .expect("to be able to convert into str"),
                );
                if let Some(file_name) = file_name {
                    vdfs.extract_file(&file_name, &file_map);
                } else {
                    println!(
                        "[INFO] Extracting {} to '{}'",
                        path.display(),
                        output_dir.display()
                    );
                    vdfs.extract_all(&output_dir, &file_map)
                        .expect("extraction to work");
                }
            }
        }
    }
    Ok(())
}
