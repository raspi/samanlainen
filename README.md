# samanlainen

![GitHub All Releases](https://img.shields.io/github/downloads/raspi/samanlainen/total?style=for-the-badge)
![GitHub release (latest by date)](https://img.shields.io/github/v/release/raspi/samanlainen?style=for-the-badge)
![GitHub tag (latest by date)](https://img.shields.io/github/v/tag/raspi/samanlainen?style=for-the-badge)

Delete duplicate files. Uses SHA512. Rewritten from [duplikaatti](https://github.com/raspi/duplikaatti) (Go) in Rust.

## Usage

```
% target/x86_64-unknown-linux-gnu/release/samanlainen --help
samanlainen 0.2.1
Pekka Järvinen
Delete duplicate files. Uses SHA512.

USAGE:
    samanlainen [OPTIONS] <PATHS>...

ARGS:
    <PATHS>...    Path(s) to scan for duplicate files

OPTIONS:
    -c, --count <COUNT>              Minimum count of files considered duplicate (min. 2) [default:
                                     2]
    -C, --color <COLOR>              Color [default: auto] [possible values: auto, off]
        --delete-files               Delete files? If enabled, files are actually deleted
    -h, --help                       Print help information
    -m, --minsize <MINSIZE>          Minimum filesize to scan, supports EIC/SI units [default: 1B]
    -M, --maxsize <MAXSIZE>          Maximum filesize to scan, supports EIC/SI units [default: 1EiB]
    -s, --scansize <SCANSIZE>        Scan size used for scanning first and last bytes of file,
                                     supports EIC/SI units [default: 1MiB]
    -S, --sort-order <SORT_ORDER>    Sort order [default: i-node] [possible values: i-node,
                                     filename, depth]
    -v, --verbose                    Be verbose, -vvv... be very verbose
    -V, --version                    Print version information
```

## Example run

```shell
% ls -la test
total 20
drwxr-xr-x 2 raspi raspi 4096  2. 7. 02:36 .
drwxr-xr-x 7 raspi raspi 4096  2. 7. 23:57 ..
-rw-r--r-- 1 raspi raspi 1000  2. 7. 02:36 random_copy2.dat
-rw-r--r-- 1 raspi raspi 1000  2. 7. 02:36 random_copy.dat
-rw-r--r-- 1 raspi raspi 1000  2. 7. 02:36 random.dat

% sha1sum test/*
105af7b371b01ee4bbb2dc7242b001dd61b07a26  test/random_copy2.dat
105af7b371b01ee4bbb2dc7242b001dd61b07a26  test/random_copy.dat
105af7b371b01ee4bbb2dc7242b001dd61b07a26  test/random.dat

% samanlainen test
Not deleting files (dry run), add --delete-files to actually delete files.
File sizes to scan: 1 B - 1152921504606846976 B (1.15 EB, 1 EiB)
Scan size for last and first bytes of files: 1048576 B (1.05 MB, 1 MiB)
Directories to scan:
 * /home/raspi/samanlainen/test

(1 / 6) Generating file list based on file sizes...
  File candidates: 3 Total size: 3000 B (3 kB, 2.93 kiB)
(2 / 6) Eliminating candidates based on last 1048576 bytes of files...
  File candidates: 3 Total size: 3000 B (3 kB, 2.93 kiB)
(3 / 6) Eliminating candidates based on first 1048576 bytes of files...
  File candidates: 3 Total size: 3000 B (3 kB, 2.93 kiB)
(4 / 6) Hashing 3 files with size 1000 B (1 kB, 1000 B)  Total: 3000 B (3 kB, 2.93 kiB)...
(5 / 6) Deleting duplicate files with checksum: 2a890a868d62a5c06a6354e023e6bf44016c963d376b2a736351348d5588f762f7199871d6a7ac09f01452846cbc45e63bd791bb30d483226203fa45f91bdca3
   +keeping: /home/raspi/samanlainen/test/random.dat
  -deleting: /home/raspi/samanlainen/test/random_copy.dat
  -deleting: /home/raspi/samanlainen/test/random_copy2.dat
Currently removed 2 files totaling 2000 B (2 kB, 1.95 kiB)  Remaining: 0 files, 0 B
(6 / 6) Removed 2 files totaling 2000 B (2 kB, 1.95 kiB)
```

## Algorithm

1. Create file list of given directories
    * do not add files with same identifier already added to the list (windows: file id, *nix: inode)
    * do not add 0 byte files
    * directories listed first has higher priority than the last
1. Remove all files from the list which do not share same file sizes (ie. there's only one 1000 byte file -> remove)
1. Read last bytes of files and generate SHA512 sum of those bytes
1. Remove all hashes from the list which occured only once
1. Read first bytes of files and generate SHA512 sum of those bytes
1. Remove all hashes from the list which occured only once
1. Now finally hash the whole files that are left
1. Remove all hashes from the list which occured only once
1. Generate list of files to keep and what to remove
    * use directory priority and file age to find what to keep
        * oldest and highest priority files are kept
1. Finally, remove files from filesystem(s)

## Is it any good?

Yes.
