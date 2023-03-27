use std::{io, iter};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

use sha2::{Digest, Sha512};
use walkdir::{DirEntryExt, WalkDir};

// Generate stats from list of files
pub fn generate_stats(l: HashMap<u64, Vec<PathBuf>>) -> (u64, u64) {
    let mut file_count: u64 = 0;
    let mut total_size: u64 = 0;

    for (fsize, files) in l {
        file_count += files.len() as u64;
        total_size += fsize * (files.len() as u64);
    }

    (file_count, total_size)
}

// Find possible duplicates based on last or first bytes of files
pub fn eliminate_first_or_last_bytes_hash(
    l: HashMap<u64, Vec<PathBuf>>,     // List of files
    t: ScanType, // Scan first or last bytes of file
    scansize: u64, // how many bytes to scan
    min_count: u64, // minimal count considered as duplicate (2 or more)
) -> io::Result<HashMap<u64, Vec<PathBuf>>> {
    if min_count < 2 {
        panic!("count < 2")
    }

    // used for generating a new list of candidate files
    let mut newl: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for (fsize, files) in l {
        if fsize <= scansize {
            // File is too small for last/first bytes hashing
            // Send for later processing
            newl.insert(fsize, files);
            continue;
        }

        let mut hashes: HashMap<String, Vec<PathBuf>> = HashMap::new();

        for file in files {
            let checksum = hash_partial(file.to_owned(), t, scansize)?;

            hashes
                .entry(checksum)
                .or_default()
                .push(file);
        }

        for (_, filelist) in hashes {
            if filelist.is_empty() {
                continue;
            }

            if filelist.len() < min_count as usize {
                // Remove if there's too few files with same hash
                continue;
            }

            for fpath in filelist {
                newl
                    .entry(fsize)
                    .or_default()
                    .push(fpath);
            }
        }
    }

    Ok(newl)
}

// Find initial candidates from given path(s)
pub fn find_candidate_files(
    paths: Vec<PathBuf>, // file path(s) to scan for files
    minimum_size: u64, // file size must be at least this
    maximum_size: u64, // file size cannot be larger than this, 0 disables max size
    count: u64, // there must be at least this many files with same file size to be considered a duplicate (must be 2 or more)
) -> io::Result<HashMap<u64, Vec<PathBuf>>> {
    if count < 2 {
        panic!("count < 2")
    }

    let mut found_inodes: Vec<u64> = Vec::new();

    // l[filesize][]filepath
    let mut sizes: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for path in paths {
        for entry in WalkDir::new(path)
            .follow_links(false)
            .same_file_system(true)
            .sort_by(|a, b|
                a.ino().cmp(&b.ino())
            ) {
            let e = entry?;

            if e.file_type().is_symlink() {
                continue;
            }

            if e.file_type().is_dir() {
                // Only files
                continue;
            }

            if !e.file_type().is_file() {
                // Only files
                continue;
            }

            let m = e.metadata()?;
            if m.len() == 0 {
                // Zero sized file, skip
                continue;
            }

            if m.len() < minimum_size {
                // Too small file
                continue;
            }

            if m.len() > maximum_size {
                // Too large file
                continue;
            }

            if found_inodes.contains(&e.ino()) {
                // Existing file with same inode, skip
                continue;
            }

            found_inodes.push(e.ino().to_owned());

            sizes
                .entry(m.len())
                .or_default()
                .push(e.into_path());
        }
    }

    // Filter out file groups which has too few files
    let mut files: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for (k, v) in sizes {
        if v.is_empty() {
            continue;
        }

        if v.len() < count as usize {
            // Too few files to be considered duplicate
            continue;
        }

        files.entry(k).or_insert(v);
    }


    Ok(files)
}

#[derive(Clone, Copy)]
pub enum ScanType {
    // Scan first N bytes
    First,
    // Scan last N bytes
    Last,
}

fn checksum_to_hex(bytes: &[u8]) -> String {
    let mut s: String = String::new();

    for b in bytes {
        s.push_str(format!("{:02x}", b).as_str());
    }

    s
}

// Hash file partially from beginning or end
fn hash_partial(
    p: PathBuf, // File to scan
    t: ScanType, // Scan first or last bytes of file
    s: u64, // how many bytes to scan
) -> io::Result<String> {
    if s == 0 {
        panic!("zero size")
    }

    let mut f = File::open(p)?;

    match t {
        ScanType::First => {
            // Do nothing
        }
        ScanType::Last => {
            // Seek from end position
            f.seek(SeekFrom::End(-(s as i64)))?;
        }
    }

    let mut buffer: Vec<u8> = iter::repeat(0u8).take(s as usize).collect();
    let mut reader = BufReader::new(f);
    let mut hasher = Sha512::new();

    let count = reader.read(&mut buffer)?;
    if count == 0 {
        panic!("0????")
    }
    hasher.update(&buffer[..count]);

    Ok(checksum_to_hex(hasher.finalize().as_slice()))
}

// Hash the entire file
fn hash_full(
    p: PathBuf, // File to scan
) -> io::Result<String> {
    let f = File::open(p)?;

    let mut buffer = [0u8; 1048576];
    let mut reader = BufReader::new(f);
    let mut hasher = Sha512::new();

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }

    Ok(checksum_to_hex(hasher.finalize().as_slice()))
}

// Hashes files fully and returns file list and checksum as the key
pub fn find_final_candidates(
    l: Vec<PathBuf>,     // List of files
) -> io::Result<HashMap<String, Vec<PathBuf>>> {
    let mut res: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut hashes: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for file in l {
        let checksum = hash_full(file.to_owned())?;


        hashes
            .entry(checksum)
            .or_default()
            .push(file);
    }


    // Filter out file groups which has too few files
    for (checksum, files) in hashes {
        if files.is_empty() {
            continue;
        }

        if files.len() < 2 {
            // Each file with same checksum must have 2 or more files to be considered duplicate
            continue;
        }

        res.insert(checksum, files);
    }

    Ok(res)
}


#[test]
fn test_integration() -> io::Result<()> {
    let mincount: u64 = 2;
    let scansize: u64 = 1048576;

    let mut paths: Vec<PathBuf> = Vec::new();
    paths.push(Path::new("test").to_path_buf());

    let mut cf = find_candidate_files(paths, 1, 0, mincount)?;
    cf = eliminate_first_or_last_bytes_hash(cf, ScanType::Last, scansize, mincount)?;
    cf = eliminate_first_or_last_bytes_hash(cf, ScanType::First, scansize, mincount)?;

    for (fsize, files) in cf {
        let final_candidates = find_final_candidates(files)?;

        for (checksum, files) in final_candidates {
            for file in files {
                println!("{}", file.display())
            }
        }
    }

    Ok(())
}

