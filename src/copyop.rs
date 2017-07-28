// Copyright (c) 2017 Jason White
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
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::path::{Path, PathBuf, Component, Prefix};

use std::fmt;
use std::io;
use std::time::Duration;
use std::ffi::OsString;

use util;

/// Returns a normalized path.
fn norm<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    let mut new_path = PathBuf::new();

    let mut components = path.components();

    if path.as_os_str().len() >= 260 {
        // If the path is >= 260 characters, we should prefix it with '\\?\' if
        // possible.
        if let Some(c) = components.next() {
            match c {
                Component::CurDir => {},
                Component::RootDir |
                Component::ParentDir |
                Component::Normal(_) => {
                    // Can't add the prefix. It's a relative path.
                    new_path.push(c.as_os_str());
                },
                Component::Prefix(prefix) => {
                    match prefix.kind() {
                        Prefix::UNC(server, share) => {
                            let mut p = OsString::from(r"\\?\UNC\");
                            p.push(server);
                            p.push(r"\");
                            p.push(share);
                            new_path.push(p);
                        },
                        Prefix::Disk(_) => {
                            let mut p = OsString::from(r"\\?\");
                            p.push(c.as_os_str());
                            new_path.push(p);
                        },
                        _ => { new_path.push(c.as_os_str()); },
                    };
                },
            };
        }
    }

    for c in components {
        match c {
            Component::CurDir => {},
            Component::ParentDir => {
                let pop = match new_path.components().next_back() {
                    Some(Component::Prefix(_)) | Some(Component::RootDir) => true,
                    Some(Component::Normal(s)) => !s.is_empty(),
                    _ => false,
                };

                if pop {
                    new_path.pop();
                }
                else {
                    new_path.push("..");
                }
            },
            _ => { new_path.push(c.as_os_str()); },
        };
    }

    if new_path.as_os_str().is_empty() {
        new_path.push(".");
    }

    new_path
}

/// A copy operation.
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CopyOp {
    pub src: PathBuf,
    pub dest: PathBuf,
}

impl fmt::Display for CopyOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{}\" -> \"{}\"",
               self.src.to_str().unwrap(), self.dest.to_str().unwrap())
    }
}

impl CopyOp {

    pub fn new<P>(from: P, to: P) -> CopyOp
        where P: AsRef<Path>
    {
        CopyOp {
            src: norm(from),
            dest: norm(to),
        }
    }

    /// Copies the source file to the given destination. It is expected that the
    /// destination directory already exists.
    pub fn copy(&self, retries: usize, retry_delay: Duration) -> io::Result<u64> {
        util::copy_retry(&self.src, &self.dest, retries, retry_delay)
    }

    /// Returns `true` if this copy operation is "complete". That is, if the
    /// copy does not need to done again. Returns an `Err` result if a copy
    /// operation *cannot* complete if attempted. That is, if the source does
    /// not exist or we do not have permissions for it. Similarly, if both the
    /// source and destinations are both files or both directories.
    pub fn is_complete(&self, retries: usize, retry_delay: Duration) -> io::Result<bool> {
        let a = try!(util::metadata_retry(&self.src, retries, retry_delay));
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
        }
        else if a.file_type() != b.file_type() {
            trace!("{}: file_type {:?} != {:?}", self, a.file_type(), b.file_type());
            Ok(false)
        }
        else if a.modified().unwrap() != b.modified().unwrap() {
            trace!("{}: modified {:?} != {:?}", self, a.modified().unwrap(), b.modified().unwrap());
            Ok(false)
        }
        else if a.permissions().readonly() != b.permissions().readonly() {
            trace!("{}: readonly {:?} != {:?}", self, a.permissions().readonly(), b.permissions().readonly());
            Ok(false)
        }
        else {
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

    #[test]
    #[cfg(windows)]
    fn test_norm() {
        assert_eq!(Path::new("../foo").parent(), Some(Path::new("..")));
        assert_eq!(norm("foo"), Path::new("foo"));
        assert_eq!(norm("./foo"), Path::new("foo"));
        assert_eq!(norm("."), Path::new("."));
        assert_eq!(norm(".."), Path::new(".."));
        assert_eq!(norm(r"..\.."), Path::new(r"..\.."));
        assert_eq!(norm(r"..\..\.."), Path::new(r"..\..\.."));
        assert_eq!(norm(""), Path::new("."));
        assert_eq!(norm("foo/bar"), Path::new(r"foo\bar"));
        assert_eq!(norm("C:/foo/../bar"), Path::new(r"C:\bar"));
        assert_eq!(norm("C:/../bar"), Path::new(r"C:\bar"));
        assert_eq!(norm("C:/../../bar"), Path::new(r"C:\bar"));
        assert_eq!(norm("foo//bar///"), Path::new(r"foo\bar"));
        assert_eq!(norm(r"\\server\share\..\foo"), Path::new(r"\\server\share\foo"));
        assert_eq!(norm(r"\\server\share\..\foo\.."), Path::new(r"\\server\share"));
        assert_eq!(norm(r"..\foo\..\..\bar"), Path::new(r"..\..\bar"));
    }

    #[test]
    #[cfg(windows)]
    fn test_norm_long_paths() {

        let long_name : String = iter::repeat('a').take(260).collect();
        let long_name = long_name.as_str();

        // Long paths
        assert_eq!(norm(String::from(r"C:\")+long_name),
                   PathBuf::from(String::from(r"\\?\C:\")+long_name));
        assert_eq!(norm(String::from(r"\\server\share\")+long_name),
                   PathBuf::from(String::from(r"\\?\UNC\server\share\")+long_name));

        // Long relative paths
        assert_eq!(norm(String::from(r"..\relative\")+long_name),
                   PathBuf::from(String::from(r"..\relative\")+long_name));
        assert_eq!(norm(String::from(r".\relative\")+long_name),
                   PathBuf::from(String::from(r"relative\")+long_name));
    }
}
