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

use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::Path;

use crate::copyop::CopyOp;

#[derive(Debug)]
pub enum Error<'a> {
    /// There are one or more paths that are common to both the source and
    /// destinations in the *next* manifest. Since source files can be copied
    /// to corresponding destinations in any order, this indicates a race
    /// condition.
    Overlap(Vec<&'a Path>),

    /// There are one or more paths that are duplicated in the destinations of
    /// the *next* manifest.
    Duplicates(Vec<(&'a Path, usize)>),

    /// There are one or more missing source files in the *next* manifest.
    /// Obviously, we can't copy what doesn't exist.
    MissingSrcs(Vec<(&'a CopyOp, io::Error)>),

    /// Some directories failed to get created.
    CreateDirs(Vec<(&'a Path, io::Error)>),

    /// There are one or more files that failed to get deleted.
    Delete(Vec<(&'a Path, io::Error)>),

    /// There are one or more directories that failed to get deleted.
    DeleteDirs(Vec<(&'a Path, io::Error)>),

    /// There are one or more files that failed to get copied.
    Copy(Vec<(&'a CopyOp, io::Error)>),

    /// There are outdated copy operations after the copy. This should never
    /// happen and indicates a bug in Ubercopy.
    VerifyIncomplete(Vec<&'a CopyOp>),

    /// There were failures when trying to determine outdated copy operations.
    /// This can happen if a source file was removed just after it was copied,
    /// but before we did the sanity check. This indicates a race condition
    /// with some other process.
    VerifyErrors(Vec<(&'a CopyOp, io::Error)>),
}

impl<'a> StdError for Error<'a> {
    fn description(&self) -> &str {
        match *self {
            Error::Overlap(_) => "Overlapping sources and destinations",
            Error::Duplicates(_) => "Duplicate destinations",
            Error::MissingSrcs(_) => {
                "Error finding out-of-date copy operations"
            }
            Error::CreateDirs(_) => "Failed to create destination directories",
            Error::Delete(_) => "Failed to delete the following files",
            Error::DeleteDirs(_) => {
                "Failed to delete the following directories"
            }
            Error::Copy(_) => "Failed to copy file(s)",
            Error::VerifyIncomplete(_) => "Verification check failed",
            Error::VerifyErrors(_) => {
                "Failed trying to perform verification check"
            }
        }
    }
}

const OVERLAP: &str = "\
Error: Some file(s) are both sources and destinations. This is a race
       condition. The files listed above are listed in the manifest as both
       sources and destinations.";

const DUPLICATES: &str = "\
Error: Duplicate destination path(s). The files listed above appear more than
       once as destinations in the manifest. This is a race condition.";

const MISSING_SOURCES: &str = "\
Error: The source file(s) listed above are either missing or have some other
       problem. Make sure these files exist and are accessible.";

const CREATE_DIRS: &str =
    "Error: The destination directories listed above failed to get created.";

const DELETE: &str =
    "Error: The above destination files failed to get deleted.";

const DELETE_DIRS: &str =
    "Error: The above destination directories failed to get deleted.";

const COPIES: &str = "Error: The copy operations listed above failed.";

const VERIFICATION_INCOMPLETE: &str = "\
Error: The copy operation(s) listed above are still incomplete even after
       copying them. This can happen if a file was modified by another process
       during the copy. Simply re-running the copy usually fixes it.";

const VERIFICATION_ERRORS: &str = "\
Error: Copy verification failed. The source file(s) listed above are either
       missing or have some other problem. This can happen if a source file was
       removed or changed somehow just after it was copied to the destination.
       This indicates a race condition with some other process. Make sure
       nothing else is messing with these files during the copy.";

impl<'a> fmt::Display for Error<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.to_string())?;

        match *self {
            Error::Overlap(ref overlap) => {
                for path in overlap {
                    writeln!(f, " - {:?}", path)?;
                }

                writeln!(f, "{}", OVERLAP)
            }
            Error::Duplicates(ref duplicates) => {
                for &(path, count) in duplicates {
                    writeln!(f, " - {:?} ({} duplicates)", path, count)?;
                }

                writeln!(f, "{}", DUPLICATES)
            }
            Error::MissingSrcs(ref errors) => {
                for &(op, ref err) in errors {
                    writeln!(f, " - {:?}: {}", op.src, err)?;
                }

                writeln!(f, "{}", MISSING_SOURCES)
            }
            Error::CreateDirs(ref errors) => {
                for &(dir, ref err) in errors {
                    writeln!(f, " - {:?}: {}", dir, err)?;
                }

                writeln!(f, "{}", CREATE_DIRS)
            }
            Error::Delete(ref failed) => {
                for &(path, ref err) in failed {
                    writeln!(f, " - {:?}: {}", path, err)?;
                }

                writeln!(f, "{}", DELETE)
            }
            Error::DeleteDirs(ref failed) => {
                for &(dir, ref err) in failed {
                    writeln!(f, " - {:?}: {}", dir, err)?;
                }

                writeln!(f, "{}", DELETE_DIRS)
            }
            Error::Copy(ref errors) => {
                for &(op, ref err) in errors {
                    writeln!(f, " - {:?} ({})", op.src, err)?;
                }

                writeln!(f, "{}", COPIES)
            }
            Error::VerifyIncomplete(ref ops) => {
                for op in ops {
                    writeln!(f, " - {}", op)?;
                }

                writeln!(f, "{}", VERIFICATION_INCOMPLETE)
            }
            Error::VerifyErrors(ref errors) => {
                for &(op, ref err) in errors {
                    writeln!(f, " - {:?} ({})", op.src, err)?;
                }

                writeln!(f, "{}", VERIFICATION_ERRORS)
            }
        }
    }
}
