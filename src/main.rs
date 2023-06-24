mod vdfs;

use clap::Parser;
use vdfs::VDFS;

#[derive(Parser, Debug)]
#[command(term_width = 0, arg_required_else_help(true))]
struct Args {
    /// The base directory
    #[arg(short = 'b', long, value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    base_directory: std::path::PathBuf,

    /// The output file
    #[arg(short = 'o', long, value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
    output_file: std::path::PathBuf,

    /// Comment to be added to the volume
    #[arg(short = 'c', long)]
    comment: Option<String>,
}

fn main() -> Result<(), std::io::Error> {
    let args = Args::parse();
    let mut vdfs = VDFS::new(&args.base_directory);
    vdfs.comment(if let Some(c) = &args.comment { c } else { "" });
    vdfs.save_to_file(&args.output_file)?;
    Ok(())
}
