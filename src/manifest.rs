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

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use crate::util::PathExt;

/// Represents a manifest. A manifest is simply a sequence of copy operations.
pub struct Manifest {
    operations: Vec<CopyOp>,
}

impl Manifest {
    pub fn new() -> Manifest {
        Manifest { operations: vec![] }
    }

    pub fn parse_reader<R, P>(
        reader: R,
        dest_dir: P,
        sandbox_src: bool,
        sandbox_dest: bool,
    ) -> Result<Self, String>
    where
        R: io::BufRead,
        P: AsRef<Path>,
    {
        let dest_dir = dest_dir.as_ref();

        let mut operations: Vec<CopyOp> = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line = line.unwrap();
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                // Ignore blank lines and comments
                continue;
            }

            let mut s = line.split('\t');

            let src = s.next().ok_or_else(|| {
                format!("Missing source file on line {}", i + 1)
            })?;
            let dest = s.next().ok_or_else(|| {
                format!("Missing destination file on line {}", i + 1)
            })?;

            let src_path = Path::new(src).norm();

            if sandbox_src && !src_path.is_sandboxed() {
                return Err(format!(
                    "source path {:?} is not sandboxed",
                    src_path
                ));
            }

            let dest_path = Path::new(dest).norm();

            if sandbox_dest && !dest_path.is_sandboxed() {
                return Err(format!(
                    "destination path {:?} is not sandboxed",
                    dest_path
                ));
            }

            let dest_path = if dest_dir.is_empty() {
                dest_path
            } else {
                let mut path = PathBuf::new();
                path.push(dest_dir);
                path.push(dest_path);
                path.norm()
            };

            operations.push(CopyOp::new(src_path, dest_path));
        }

        // This vector needs to be sorted so that we can diff two manifests.
        operations.sort();

        // It is fine for a manifest to have duplicate copy operations. Remove
        // them here so that we don't get errors about duplicate destinations.
        operations.dedup();

        Ok(Manifest { operations })
    }

    pub fn parse<P>(
        path: P,
        dest: P,
        sandbox_src: bool,
        sandbox_dest: bool,
    ) -> Result<Self, String>
    where
        P: AsRef<Path>,
    {
        let f = File::open(path).map_err(|e| e.to_string())?;
        Manifest::parse_reader(
            io::BufReader::new(f),
            dest,
            sandbox_src,
            sandbox_dest,
        )
    }

    /// Returns a sorted list of all sources.
    pub fn srcs(&self) -> Vec<&Path> {
        self.operations()
            .iter()
            .map(|op| op.src.as_path())
            .collect()
    }

    /// Returns a sorted list of all destinations.
    pub fn dests(&self) -> Vec<&Path> {
        let mut dests: Vec<&Path> = self
            .operations()
            .iter()
            .map(|op| op.dest.as_path())
            .collect();
        dests.sort();
        dests
    }

    /// List of all copy operations in this manifest.
    pub fn operations(&self) -> &Vec<CopyOp> {
        &self.operations
    }

    /// List of copy operations that need to occur in order to bring the
    /// destinations up-to-date. This also checks if the source location exists.
    /// If not, then an error result for that copy operation is returned.
    pub fn outdated(
        &self,
        force: bool,
        pool: &Pool,
        retries: usize,
        retry_delay: Duration,
    ) -> Result<Vec<&CopyOp>, Vec<(&CopyOp, io::Error)>> {
        log::info!("Finding list of outdated copy operations");

        if force {
            // Assume all files need to be copied.
            return Ok(self.operations.iter().collect());
        }

        let (tx, rx) = sync_channel(32);

        let (errors, result) = pool.scoped(|scope| {
            for op in &self.operations {
                let tx = tx.clone();
                scope.execute(move || {
                    tx.send((op, op.is_complete(retries, retry_delay)))
                        .unwrap();
                });
            }

            let mut errors: Vec<(&CopyOp, io::Error)> = Vec::new();
            let mut result: Vec<&CopyOp> = Vec::new();

            for (op, complete) in rx.iter().take(self.operations.len()) {
                match complete {
                    Ok(false) => result.push(op),
                    Ok(true) => {}
                    Err(err) => errors.push((op, err)),
                };
            }

            (errors, result)
        });

        if errors.is_empty() {
            log::info!("Found {} outdated copy operations", result.len());
            Ok(result)
        } else {
            Err(errors)
        }
    }
}
