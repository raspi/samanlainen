use std::{cmp, io};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Error;
use std::fs::{canonicalize, remove_file};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;

use clap::{App, Arg, ArgAction};
use clap::Parser;
use parse_size::parse_size;
use walkdir::{DirEntry, DirEntryExt};

use samanlainen_lib::{
    eliminate_first_or_last_bytes_hash, find_candidate_files, find_final_candidates,
    generate_stats, ScanType,
};

#[derive(Clone, Copy)]
enum ConvertTo {
    // 1000
    SI,
    // 1024
    IEC,
}

fn convert_to_human(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{} B", bytes);
    }

    format!(
        "{} B ({}, {})",
        bytes,
        convert_bytes(bytes, ConvertTo::SI),
        convert_bytes(bytes, ConvertTo::IEC)
    )
}

fn convert_bytes(bytes: u64, conv: ConvertTo) -> String {
    let num: f64 = bytes as f64;

    let units = match conv {
        ConvertTo::SI => ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"],
        ConvertTo::IEC => ["B", "kiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"],
    };

    if num < 1_f64 {
        return format!("{} {}", num, units[0]);
    }

    let delimiter = match conv {
        ConvertTo::SI => 1000_f64,
        ConvertTo::IEC => 1024_f64,
    };

    let exponent = cmp::min(
        (num.ln() / delimiter.ln()).floor() as i32,
        (units.len() - 1) as i32,
    );

    let pretty_bytes = format!("{:.2}", num / delimiter.powi(exponent))
        .parse::<f64>()
        .unwrap()
        * 1_f64;
    format!("{} {}", pretty_bytes, units[exponent as usize])
}

fn parse_min_bytes(s: &str) -> Result<u64, Error> {
    let min = parse_size(s).expect("could not parse");

    if min < 1 {
        panic!("minimum is 1 for minimum size");
    }

    Ok(min)
}

fn parse_max_bytes(s: &str) -> Result<u64, Error> {
    let max = parse_size(s).expect("could not parse");

    if max < 1 {
        panic!("minimum is 1 for maximum size");
    }

    Ok(max)
}

fn parse_scansize_bytes(s: &str) -> Result<u64, Error> {
    let ss = parse_size(s).expect("could not parse");

    if ss < 1 {
        panic!("minimum is 1 for scan size");
    }

    Ok(ss)
}

// CLI arguments
// See: https://docs.rs/clap/latest/clap/
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CLIArgs {
    #[clap(
        short = 'v',
        long,
        parse(from_occurrences),
        help = "Be verbose, -vvv... be very verbose"
    )]
    verbose: u64,

    #[clap(short = 'm', long, default_value = "1B",
    help = "Minimum filesize to scan",
    parse(try_from_str = parse_min_bytes))]
    minsize: u64,

    #[clap(short = 'M', long, default_value = "1EiB",
    help = "Maximum filesize to scan",
    parse(try_from_str = parse_max_bytes))]
    maxsize: u64,

    #[clap(short = 'c', long, default_value = "2",
    help = "Minimum count of files considered duplicate (min. 2)",
    value_parser = clap::value_parser ! (u64).range(2..))]
    count: u64,

    #[clap(short = 's', long, default_value = "1MiB",
    help = "Scan size used for scanning first and last bytes of file",
    parse(try_from_str = parse_scansize_bytes))]
    scansize: u64,

    #[clap(long, help = "Delete files? If enabled, files are actually deleted")]
    delete_files: bool,

    #[clap(
        short = 'S',
        long,
        value_enum,
        help = "Sort order",
        default_value = "i-node"
    )]
    sort_order: DirSortOrder,

    #[clap(
        required = true,
        multiple = true,
        help = "Path(s) to scan for duplicate files"
    )]
    paths: Vec<PathBuf>,
}

fn get_directories(dirs: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
    let mut found_dirs: Vec<PathBuf> = Vec::new(); // for possible duplicates
    let mut dirs_to_search: Vec<PathBuf> = Vec::new();

    for dir in dirs {
        // Convert to absolute path
        let path = canonicalize(Path::new(&dir))?;

        if found_dirs.contains(&path.to_path_buf()) {
            continue;
        }

        if path.is_dir() {
            found_dirs.push(path.to_path_buf());
            dirs_to_search.push(path.to_owned());
        } else {
            panic!("ERROR: Not a directory: {}", path.display());
        }
    }

    Ok(dirs_to_search)
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum DirSortOrder {
    INode,
    Filename,
    Depth,
}

fn cmp_inode(a: &DirEntry, b: &DirEntry) -> Ordering {
    a.ino().cmp(&b.ino())
}

fn cmp_filename(a: &DirEntry, b: &DirEntry) -> Ordering {
    a.file_name().cmp(b.file_name())
}

fn cmp_depth(a: &DirEntry, b: &DirEntry) -> Ordering {
    a.depth().cmp(&b.depth())
}

fn main() -> Result<(), io::Error> {
    let args: CLIArgs = CLIArgs::parse();

    if args.minsize > args.maxsize {
        println!("minsize is larger than maxsize");
        exit(1);
    }

    if args.maxsize < args.minsize {
        println!("maxsize is smaller than minsize");
        exit(1);
    }

    let dirs_to_search: Vec<PathBuf> = get_directories(args.paths)?;

    if dirs_to_search.is_empty() {
        println!("No directories");
        exit(0);
    }

    if args.delete_files {
        println!("WARNING: deleting files!");
    } else {
        println!("Not deleting files (dry run), add --delete-files to actually delete files.");
    }

    println!();

    println!(
        "File sizes to scan: {} - {}",
        convert_to_human(args.minsize),
        convert_to_human(args.maxsize)
    );

    println!(
        "Scan size for last and first bytes of files: {}",
        convert_to_human(args.scansize)
    );

    println!("Directories to scan:");
    for dir in dirs_to_search.clone() {
        println!(" * {}", dir.display());
    }

    println!();

    println!("(1 / 6) Generating file list based on file sizes...");

    let cmp = match args.sort_order {
        DirSortOrder::INode => cmp_inode,
        DirSortOrder::Filename => cmp_filename,
        DirSortOrder::Depth => cmp_depth,
    };

    let mut files_found: HashMap<u64, Vec<PathBuf>> =
        find_candidate_files(dirs_to_search, args.minsize, args.maxsize, args.count, cmp)?;
    let (file_count, total_size) = generate_stats(files_found.to_owned());
    println!(
        "  File candidates: {} Total size: {}",
        file_count,
        convert_to_human(total_size)
    );
    if files_found.is_empty() {
        println!("No files.");
        exit(0);
    }

    // Scan last bytes
    println!(
        "(2 / 6) Eliminating candidates based on last {} bytes of files...",
        args.scansize
    );
    files_found = eliminate_first_or_last_bytes_hash(
        files_found.to_owned(),
        ScanType::Last,
        args.scansize,
        args.count,
    )?;
    let (file_count, total_size) = generate_stats(files_found.to_owned());
    println!(
        "  File candidates: {} Total size: {}",
        file_count,
        convert_to_human(total_size)
    );
    if files_found.is_empty() {
        println!("No files.");
        exit(0);
    }

    // Scan first bytes
    println!(
        "(3 / 6) Eliminating candidates based on first {} bytes of files...",
        args.scansize
    );
    files_found = eliminate_first_or_last_bytes_hash(
        files_found.to_owned(),
        ScanType::First,
        args.scansize,
        args.count,
    )?;
    let (file_count, total_size) = generate_stats(files_found.to_owned());
    println!(
        "  File candidates: {} Total size: {}",
        file_count,
        convert_to_human(total_size)
    );
    if files_found.is_empty() {
        println!("No files.");
        exit(0);
    }

    let mut freed_space: u64 = 0;
    let mut freed_files: u64 = 0;
    let mut files_remaining: u64 = file_count;
    let mut space_remaining: u64 = total_size;

    // remove files in file size groups so that collision with different sized files are less likely
    for (fsize, files) in files_found {
        if files.is_empty() {
            continue;
        }

        files_remaining -= files.len() as u64;
        space_remaining -= fsize * (files.len() as u64);

        println!(
            "(4 / 6) Hashing {} files with size {}  Total: {}...",
            files.len(),
            convert_to_human(fsize),
            convert_to_human(fsize * (files.len() as u64))
        );
        let final_candidates = find_final_candidates(files)?;

        for (checksum, files) in final_candidates {
            if files.is_empty() {
                println!("  There were no files");
                continue;
            }

            if (files.len() as u64) < args.count {
                println!(
                    "  There were too few files with same checksum ({})",
                    files.len()
                );
                continue;
            }

            println!(
                "(5 / 6) Deleting duplicate files with checksum: {}",
                checksum
            );

            for (i, file) in files.iter().enumerate() {
                if i == 0 {
                    // Keep first
                    println!("   +keeping: {}", file.display());
                    continue;
                }

                freed_space += fsize;
                freed_files += 1;

                println!("  -deleting: {}", file.display());

                if args.delete_files {
                    // actually delete file
                    remove_file(file)?;
                }
            }
        }

        println!(
            "Currently removed {} files totaling {}  Remaining: {} files, {}",
            freed_files,
            convert_to_human(freed_space),
            files_remaining,
            convert_to_human(space_remaining)
        );
    }

    println!();
    println!(
        "(6 / 6) Removed {} files totaling {}",
        freed_files,
        convert_to_human(freed_space)
    );

    Ok(())
}
