// Copyright (c) 2019 Jason White
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use scoped_pool::Pool;

use crate::copyop::CopyOp;
use crate::manifest::Manifest;

use crate::iter::{Change, IterExt};
use std::fs;
use std::io;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use crate::error::Error;
use crate::util;
use crate::util::PathExt;
use std::path::Path;

use log;

/// Returns an Error result if there are race conditions. Assumes `next_srcs`
/// and `next_dests` are sorted.
fn check_races<'a>(
    next_srcs: &[&'a Path],
    next_dests: &[&'a Path],
) -> Result<(), Error<'a>> {
    let overlap: Vec<_> = next_srcs
        .iter()
        .changes(next_dests.iter())
        .filter(|&(_, ref c)| c == &Change::None)
        .map(|(e, _)| *e)
        .collect();

    if !overlap.is_empty() {
        return Err(Error::Overlap(overlap));
    }

    let duplicates: Vec<_> = next_dests
        .iter()
        .adjacent()
        .filter(|&(_, ref count)| *count > 1) // Duplicates
        .map(|(e, ref count)| (*e, *count))
        .collect();

    if !duplicates.is_empty() {
        return Err(Error::Duplicates(duplicates));
    }

    Ok(())
}

/// Synchronizes the file system with the `next` manifest. The `prev` manifest
/// is used to calculate structural changes (e.g., files that have been
/// removed).
///
/// The synchronization takes place in several phases:
///
///  1. Check for race conditions.
///     (a) Check for overlap between the source and destination files in the
///         `next` manifest.
///     (b) Check for destination paths that have been duplicated in the `next`
///         manifest.
///  2. Compare the destinations of `prev` with that of `next` to see which ones
///     need to be deleted from disk.
///     (a) For each of the files that needs to be deleted.
///     (b) Get the parent directory for each file and delete as much as we can.
///         `rmdir` will fail if a directory isn't empty.
///  3. Compare the timestamps of the source and destination paths in `next` to
///     build up a list of copy operations that need to occur. If `--force` was
///     specified, this list should simply be the entire list in the manifest.
///     At the same time, we find out if any source files are missing.
///  4. Create parent directories for each file.
///  5. Go through the list in #3 and do the copy. Build up a list of the
///     failures and report the error.
///  6. Do a sanity check (if `sanity == true`) to make sure all timestamps are
///     equal and that all files exist. This is to help catch bugs in this
///     program.
#[allow(clippy::too_many_arguments)]
pub fn sync<'a>(
    prev: &'a Manifest,
    next: &'a Manifest,
    dryrun: bool,
    force: bool,
    verify_copy: bool,
    threads: usize,
    retries: usize,
    retry_delay: Duration,
) -> Result<usize, Error<'a>> {
    log::info!("Creating thread pool with {} threads", threads);

    let pool = Pool::new(threads);

    let prev_dests = prev.dests();
    let next_srcs = next.srcs();
    let next_dests = next.dests();

    // 1. Check for race conditions.
    log::info!("Checking for race conditions");
    check_races(&next_srcs, &next_dests)?;

    // 2. Compare the destinations of `prev` with that of `next` to see which
    //    ones need to be deleted from disk.
    let to_delete: Vec<&Path> = prev_dests
        .iter()
        .changes(next_dests.iter())
        .filter(|&(_, ref c)| c == &Change::Removed)
        .map(|(e, _)| *e)
        .collect();

    if dryrun {
        for f in &to_delete {
            log::debug!("Deleting destination {:?}", f);
        }
    } else {
        // TODO: Move all this to a separate function.
        let (tx, rx) = sync_channel(32);

        let failed = pool.scoped(|scope| {
            for f in &to_delete {
                log::debug!("Deleting destination {:?}", f);

                let tx = tx.clone();
                scope.execute(move || {
                    tx.send((
                        *f,
                        util::remove_file_retry(f, retries, retry_delay),
                    ))
                    .unwrap();
                });
            }

            let mut failed: Vec<(&'a Path, io::Error)> = Vec::new();

            for (f, result) in rx.iter().take(to_delete.len()) {
                if let Err(err) = result {
                    failed.push((f, err));
                }
            }

            failed
        });

        if !failed.is_empty() {
            return Err(Error::Delete(failed));
        }
    }

    {
        let mut failed: Vec<(&Path, io::Error)> = Vec::new();

        // Try deleting parent directories as well.
        let parent_dirs = to_delete
            .iter()
            .filter_map(|p| p.removable_parent())
            .unique();

        for dir in parent_dirs {
            log::debug!("Deleting directory {:?}", dir);

            if !dryrun {
                if let Err(error) =
                    util::remove_empty_dirs(dir, retries, retry_delay)
                {
                    failed.push((dir, error));
                }
            }
        }

        if !failed.is_empty() {
            return Err(Error::DeleteDirs(failed));
        }
    }

    // 3. Filter the manifest for files that need to be copied.
    let outdated = next.outdated(force, &pool, retries, retry_delay);

    if let Err(errors) = outdated {
        return Err(Error::MissingSrcs(errors));
    }

    let outdated = outdated.unwrap();

    {
        // 4. Create parent directories for modified files.
        let mut dirs: Vec<&Path> = outdated
            .iter()
            .filter_map(|op| op.dest.removable_parent())
            .collect();

        dirs.sort();

        let dirs: Vec<&Path> = dirs.iter().unique().cloned().collect();

        let mut failed: Vec<(&'a Path, io::Error)> = Vec::new();

        for dir in dirs {
            log::debug!("Creating directory {:?}", dir);

            if !dryrun {
                if let Err(err) = fs::create_dir_all(dir) {
                    failed.push((dir, err));
                }
            }
        }

        if !failed.is_empty() {
            return Err(Error::CreateDirs(failed));
        }
    }

    // 5. Do the actual copy.
    log::info!("Copying files...");

    if dryrun {
        for op in &outdated {
            log::debug!("Copying {}", op);
        }
    } else {
        let (tx, rx) = sync_channel(32);

        let failed = pool.scoped(|scope| {
            for op in &outdated {
                log::debug!("Copying {}", op);

                let tx = tx.clone();

                scope.execute(move || {
                    tx.send((*op, op.copy(retries, retry_delay))).unwrap();
                });
            }

            let mut failed: Vec<(&CopyOp, io::Error)> = Vec::new();

            for (op, result) in rx.iter().take(outdated.len()) {
                if let Err(err) = result {
                    failed.push((op, err));
                }
            }

            failed
        });

        if !failed.is_empty() {
            return Err(Error::Copy(failed));
        }
    }

    // 6. Verify all files have been copied successfully.
    if verify_copy && !dryrun {
        log::info!("Performing post-copy verification");

        // There should be *no* outdated files at this point.
        match next.outdated(false, &pool, retries, retry_delay) {
            Ok(ops) => {
                if !ops.is_empty() {
                    return Err(Error::VerifyIncomplete(ops));
                }
            }
            Err(errors) => return Err(Error::VerifyErrors(errors)),
        };
    }

    Ok(outdated.len())
}
