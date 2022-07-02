# samanlainen

Delete duplicate files. Uses SHA512. Rewritten from [duplikaatti](https://github.com/raspi/duplikaatti) (Go) in Rust.

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