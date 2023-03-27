use std::{cmp, io};
use std::collections::HashMap;
use std::fs::{canonicalize, remove_file};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;

use atty;
use clap::error::ErrorKind;
use clap::Parser;
use parse_size::parse_size;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use samanlainen::{
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

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ColorMode {
    Auto,
    Off,
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

fn parse_min_bytes(s: &str) -> Result<u64, clap::Error> {
    let min: u64 = match parse_size(s) {
        Ok(r) => r,
        Err(ref e) => return Err(clap::Error::raw(ErrorKind::InvalidValue, "invalid value")),
    };

    if min < 1 {
        return Err(clap::Error::raw(ErrorKind::ValueValidation, "minimum is 1 for minimum size"));
    }

    Ok(min)
}

fn parse_max_bytes(s: &str) -> Result<u64, clap::Error> {
    let max: u64 = match parse_size(s) {
        Ok(r) => r,
        Err(ref e) => return Err(clap::Error::raw(ErrorKind::InvalidValue, "invalid value")),
    };

    if max < 1 {
        return Err(clap::Error::raw(ErrorKind::ValueValidation, "minimum is 1 for maximum size"));
    }

    Ok(max)
}

fn parse_scansize_bytes(s: &str) -> Result<u64, clap::Error> {
    let ss = match parse_size(s) {
        Ok(r) => r,
        Err(ref e) => return Err(clap::Error::raw(ErrorKind::InvalidValue, "invalid value")),
    };

    if ss < 1 {
        return Err(clap::Error::raw(ErrorKind::ValueValidation, "minimum is 1 for scan size"));
    }

    Ok(ss)
}

// CLI arguments
// See: https://docs.rs/clap/latest/clap/
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CLIArgs {
    #[clap(short = 'm', long, default_value = "1B",
    help = "Minimum filesize to scan, supports EIC/SI units",
    value_parser = parse_min_bytes)]
    minsize: u64,

    #[clap(short = 'M', long, default_value = "1EiB",
    help = "Maximum filesize to scan, supports EIC/SI units",
    value_parser = parse_max_bytes)]
    maxsize: u64,

    #[clap(short = 'c', long, default_value = "2",
    help = "Minimum count of files considered duplicate (min. 2)",
    value_parser = clap::value_parser ! (u64).range(2..))]
    count: u64,

    #[clap(short = 's', long, default_value = "1MiB",
    help = "Scan size used for scanning first and last bytes of file, supports EIC/SI units",
    value_parser = parse_scansize_bytes)]
    scansize: u64,

    #[clap(long, help = "Delete files? If enabled, files are actually deleted")]
    delete_files: bool,

    #[clap(short = 'C', long, value_enum, help = "Color", default_value = "auto")]
    color: ColorMode,

    #[clap(
    help = "Path(s) to scan for duplicate files",
    required = true)]
    paths: Vec<PathBuf>,
}

fn get_directories(dirs: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    let mut found_dirs: Vec<PathBuf> = Vec::new(); // for possible duplicates
    let mut dirs_to_search: Vec<PathBuf> = Vec::new();

    for dir in dirs {
        // Convert to absolute path
        let path = match canonicalize(Path::new(&dir)) {
            Ok(r) => r,
            Err(e) => return Err(format!("{}", e).to_string()),
        };

        if found_dirs.contains(&path.to_path_buf()) {
            continue;
        }

        if path.is_dir() {
            found_dirs.push(path.to_path_buf());
            dirs_to_search.push(path.to_owned());
        } else {
            return Err(format!("ERROR: Not a directory: {}", path.display()));
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

fn main() -> Result<(), io::Error> {
    let args: CLIArgs = CLIArgs::parse();

    let color_choice = match args.color {
        ColorMode::Auto => {
            if atty::is(atty::Stream::Stdout) {
                ColorChoice::Auto
            } else {
                ColorChoice::Never
            }
        }
        ColorMode::Off => ColorChoice::Never,
    };

    const STATS_COLOR: Option<Color> = Some(Color::Rgb(160, 160, 160));
    const DEFAULT_COLOR: Option<Color> = Some(Color::Rgb(240, 240, 240));
    const ERR_COLOR: Option<Color> = Some(Color::Rgb(255, 0, 0));

    let mut stdout = StandardStream::stdout(color_choice);
    set_color(&mut stdout, DEFAULT_COLOR);

    let mut stderr = StandardStream::stderr(color_choice);
    set_color(&mut stderr, ERR_COLOR);

    if args.minsize > args.maxsize {
        writeln!(&mut stderr, "minsize is larger than maxsize").expect("");
        exit(1);
    }

    if args.maxsize < args.minsize {
        writeln!(&mut stderr, "maxsize is smaller than minsize").expect("");
        exit(1);
    }

    let dirs_to_search: Vec<PathBuf> = match get_directories(args.paths) {
        Ok(l) => l,
        Err(e) => {
            writeln!(&mut stderr, "could not parse paths: {}", e).expect("");
            exit(1);
        }
    };

    if dirs_to_search.is_empty() {
        writeln!(&mut stderr, "No directories").expect("");
        exit(0);
    }

    set_color(&mut stdout, ERR_COLOR);

    if args.delete_files {
        writeln!(&mut stdout, "WARNING: deleting files!").expect("");
    } else {
        writeln!(
            &mut stdout,
            "Not deleting files (dry run), add --delete-files to actually delete files."
        )
            .expect("");
    }

    set_color(&mut stdout, Some(Color::Rgb(128, 128, 0)));

    writeln!(
        &mut stdout,
        "File sizes to scan: {} - {}",
        convert_to_human(args.minsize),
        convert_to_human(args.maxsize)
    )
        .expect("");

    writeln!(
        &mut stdout,
        "Scan size for last and first bytes of files: {}",
        convert_to_human(args.scansize)
    )
        .expect("");

    writeln!(&mut stdout, "Directories to scan:").expect("");
    set_color(&mut stdout, Some(Color::Rgb(255, 255, 0)));
    for dir in dirs_to_search.clone() {
        writeln!(&mut stdout, " * {}", dir.display()).expect("");
    }

    writeln!(&mut stdout, "").expect("");

    set_color(&mut stdout, DEFAULT_COLOR);

    writeln!(
        &mut stdout,
        "(1 / 6) Generating file list based on file sizes..."
    )
        .expect("");

    let mut files_found: HashMap<u64, Vec<PathBuf>> =
        find_candidate_files(dirs_to_search, args.minsize, args.maxsize, args.count)?;
    let (file_count, total_size) = generate_stats(files_found.to_owned());

    set_color(&mut stdout, STATS_COLOR);
    writeln!(
        &mut stdout,
        "  File candidates: {} Total size: {}",
        file_count,
        convert_to_human(total_size)
    )
        .expect("");
    set_color(&mut stdout, DEFAULT_COLOR);

    if files_found.is_empty() {
        writeln!(&mut stdout, "No files.").expect("");
        exit(0);
    }

    // Scan last bytes
    writeln!(
        &mut stdout,
        "(2 / 6) Eliminating candidates based on last {} bytes of files  Total scan: {}...",
        convert_to_human(args.scansize),
        convert_to_human(file_count * args.scansize),
    )
        .expect("");
    files_found = eliminate_first_or_last_bytes_hash(
        files_found.to_owned(),
        ScanType::Last,
        args.scansize,
        args.count,
    )?;
    let (file_count, total_size) = generate_stats(files_found.to_owned());

    set_color(&mut stdout, STATS_COLOR);
    writeln!(
        &mut stdout,
        "  File candidates: {} Total size: {}",
        file_count,
        convert_to_human(total_size)
    )
        .expect("");
    set_color(&mut stdout, DEFAULT_COLOR);

    if files_found.is_empty() {
        writeln!(&mut stdout, "No files.").expect("");
        exit(0);
    }

    // Scan first bytes
    writeln!(
        &mut stdout,
        "(3 / 6) Eliminating candidates based on first {} bytes of files  Total scan: {}...",
        convert_to_human(args.scansize),
        convert_to_human(file_count * args.scansize),
    )
        .expect("");
    files_found = eliminate_first_or_last_bytes_hash(
        files_found.to_owned(),
        ScanType::First,
        args.scansize,
        args.count,
    )?;
    let (file_count, total_size) = generate_stats(files_found.to_owned());
    set_color(&mut stdout, STATS_COLOR);
    writeln!(
        &mut stdout,
        "  File candidates: {} Total size: {}",
        file_count,
        convert_to_human(total_size)
    )
        .expect("");
    set_color(&mut stdout, DEFAULT_COLOR);

    if files_found.is_empty() {
        writeln!(&mut stdout, "No files.").expect("");
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

        set_color(&mut stdout, DEFAULT_COLOR);

        writeln!(
            &mut stdout,
            "(4 / 6) Hashing {} files with size {}  Total: {}...",
            files.len(),
            convert_to_human(fsize),
            convert_to_human(fsize * (files.len() as u64))
        )
            .expect("");
        let final_candidates = find_final_candidates(files)?;

        for (checksum, files) in final_candidates {
            if files.is_empty() {
                writeln!(&mut stdout, "  There were no files").expect("");
                continue;
            }

            if (files.len() as u64) < args.count {
                writeln!(
                    &mut stdout,
                    "  There were too few files with same checksum ({})",
                    files.len()
                )
                    .expect("");
                continue;
            }

            writeln!(
                &mut stdout,
                "(5 / 6) Deleting duplicate files with checksum: {}",
                checksum
            )
                .expect("");

            for (i, file) in files.iter().enumerate() {
                if i == 0 {
                    // Keep first
                    set_color(&mut stdout, Some(Color::Rgb(0, 240, 0)));
                    writeln!(&mut stdout, "   +keeping: {}", file.display()).expect("");
                    continue;
                }

                freed_space += fsize;
                freed_files += 1;

                set_color(&mut stdout, Some(Color::Rgb(240, 0, 0)));
                writeln!(&mut stdout, "  -deleting: {}", file.display()).expect("");

                if args.delete_files {
                    // actually delete file
                    remove_file(file)?;
                }
            }
        }

        set_color(&mut stdout, STATS_COLOR);
        writeln!(
            &mut stdout,
            "Currently removed {} files totaling {}  Remaining: {} files, {}",
            freed_files,
            convert_to_human(freed_space),
            files_remaining,
            convert_to_human(space_remaining)
        )
            .expect("");
    }

    set_color(&mut stdout, DEFAULT_COLOR);

    writeln!(
        &mut stdout,
        "(6 / 6) Removed {} files totaling {}",
        freed_files,
        convert_to_human(freed_space)
    )
        .expect("");

    Ok(())
}

fn set_color(target: &mut StandardStream, color: Option<Color>) {
    target
        .set_color(ColorSpec::new().set_fg(color))
        .expect("couldn't set color");
}
