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

use std::path::PathBuf;

use std::fmt;
use std::io;
use std::time::Duration;

use util;

/// A copy operation.
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CopyOp {
    pub src: PathBuf,
    pub dest: PathBuf,
}

impl fmt::Display for CopyOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "\"{}\" -> \"{}\"",
            self.src.to_str().unwrap(),
            self.dest.to_str().unwrap()
        )
    }
}

impl CopyOp {
    pub fn new(from: PathBuf, to: PathBuf) -> CopyOp {
        CopyOp {
            src: from,
            dest: to,
        }
    }

    /// Copies the source file to the given destination. It is expected that the
    /// destination directory already exists.
    pub fn copy(
        &self,
        retries: usize,
        retry_delay: Duration,
    ) -> io::Result<u64> {
        util::copy_retry(&self.src, &self.dest, retries, retry_delay)
    }

    /// Returns `true` if this copy operation is "complete". That is, if the
    /// copy does not need to done again. Returns an `Err` result if a copy
    /// operation *cannot* complete if attempted. That is, if the source does
    /// not exist or we do not have permissions for it. Similarly, if both the
    /// source and destinations are both files or both directories.
    pub fn is_complete(
        &self,
        retries: usize,
        retry_delay: Duration,
    ) -> io::Result<bool> {
        let a = util::metadata_retry(&self.src, retries, retry_delay)?;
        let b = util::metadata_retry(&self.dest, retries, retry_delay);

        if b.is_err() {
            // The destination file probably doesn't exist. The copy needs to
            // happen in this case.
            return Ok(false);
        }

        let b = b.unwrap();

        // All of these must be the same in order for the copy operation to be
        // "complete".
        if a.len() != b.len() {
            trace!("{}: length {} != {}", self, a.len(), b.len());
            Ok(false)
        } else if a.file_type() != b.file_type() {
            trace!(
                "{}: file_type {:?} != {:?}",
                self,
                a.file_type(),
                b.file_type()
            );
            Ok(false)
        } else if a.modified().unwrap() != b.modified().unwrap() {
            trace!(
                "{}: modified {:?} != {:?}",
                self,
                a.modified().unwrap(),
                b.modified().unwrap()
            );
            Ok(false)
        } else if a.permissions().readonly() != b.permissions().readonly() {
            trace!(
                "{}: readonly {:?} != {:?}",
                self,
                a.permissions().readonly(),
                b.permissions().readonly()
            );
            Ok(false)
        } else {
            Ok(true)
        }
    }
}
