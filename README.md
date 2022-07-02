# samanlainen

Delete duplicate files. Uses SHA512. Rewritten from [duplikaatti](https://github.com/raspi/duplikaatti) (Go) in Rust.

## Example run

```shell
% ls -la test
total 20
drwxr-xr-x 2 raspi raspi 4096  2. 7. 02:36 .
drwxr-xr-x 7 raspi raspi 4096  2. 7. 23:57 ..
-rw-r--r-- 1 raspi raspi 1000  2. 7. 02:36 random_copy2.dat
-rw-r--r-- 1 raspi raspi 1000  2. 7. 02:36 random_copy.dat
-rw-r--r-- 1 raspi raspi 1000  2. 7. 02:36 random.dat

% samanlainen test
Not deleting files (dry run), add --delete-files to actually delete files.

File sizes to scan: 1 B - no limit
Scan size: 1048576 B (1.05 MB, 1 MiB)
Directories to scan:
  /home/raspi/samanlainen/test

(1 / 6) Generating file list based on file sizes...
  File candidates: 3 Total size: 3000 B (3 kB, 2.93 kiB)
(2 / 6) Eliminating candidates based on last 1048576 bytes of files...
  File candidates: 3 Total size: 3000 B (3 kB, 2.93 kiB)
(3 / 6) Eliminating candidates based on first 1048576 bytes of files...
  File candidates: 3 Total size: 3000 B (3 kB, 2.93 kiB)
(4 / 6) Hashing 3 files with size 1000 B (1 kB, 1000 B)...
(5 / 6) Deleting duplicate files with checksum: 2a890a868d62a5c06a6354e023e6bf44016c963d376b2a736351348d5588f762f7199871d6a7ac09f01452846cbc45e63bd791bb30d483226203fa45f91bdca3
   +keeping: /home/raspi/samanlainen/test/random.dat
  -deleting: /home/raspi/samanlainen/test/random_copy.dat
  -deleting: /home/raspi/samanlainen/test/random_copy2.dat
Currently removed 2 files totaling 2000 B (2 kB, 1.95 kiB)

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
