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
use std::fs;
use std::io;
use std::ffi;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use kernel32;
#[cfg(windows)]
use winapi::fileapi::INVALID_FILE_ATTRIBUTES;
#[cfg(windows)]
use winapi::winnt::{FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_HIDDEN};
#[cfg(windows)]
use winapi::winerror;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(any(target_os = "linux", target_os = "emscripten"))]
use libc::{stat64, lstat64, utimensat, timespec, AT_FDCWD};
#[cfg(all(unix, not(any(target_os = "linux", target_os = "emscripten"))))]
use libc::{stat as stat64, lstat as lstat64, utimensat, timespec, AT_FDCWD};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::mem;
#[cfg(unix)]
use libc::{ENOENT, ENOTEMPTY};

/// Convert a string to UTF-16.
#[cfg(windows)]
fn to_u16s<S: AsRef<ffi::OsStr>>(s: S) -> Vec<u16> {
    let mut s: Vec<u16> = s.as_ref().encode_wide().collect();
    s.push(0);
    s
}

/// Wrapper for `remove_dir` to ignore certain types of errors. The value of the
/// result indicates whether or not we can keep climbing the tree to delete more
/// parent directories.
#[cfg(windows)]
pub fn remove_dir(path: &Path) -> io::Result<bool> {
    match fs::remove_dir(path) {
        Err(err) => {
            match err.raw_os_error().unwrap() as u32 {
                winerror::ERROR_FILE_NOT_FOUND => Ok(true),
                winerror::ERROR_DIR_NOT_EMPTY => Ok(false),
                _ => Err(err),
            }
        }
        Ok(()) => Ok(true),
    }
}

#[cfg(unix)]
pub fn remove_dir(path: &Path) -> io::Result<bool> {
    match fs::remove_dir(path) {
        Err(err) => {
            match err.raw_os_error().unwrap() {
                ENOENT => Ok(true),
                ENOTEMPTY => Ok(false),
                _ => Err(err),
            }
        }
        Ok(()) => Ok(true),
    }
}

/// Remove a directory with a retry.
pub fn remove_dir_retry(
    path: &Path,
    retries: usize,
    delay: Duration,
) -> io::Result<bool> {
    match remove_dir(path) {
        Err(err) => {
            if retries > 0 {
                thread::sleep(delay);
                remove_dir_retry(path, retries - 1, delay * 2)
            } else {
                Err(err)
            }
        }
        Ok(x) => Ok(x),
    }
}

/// Deletes a directory and all its parent directories until it reaches a
/// directory that is not empty.
pub fn remove_empty_dirs(
    path: &Path,
    retries: usize,
    delay: Duration,
) -> io::Result<()> {
    if !remove_dir_retry(path, retries, delay)? {
        return Ok(());
    }

    if let Some(p) = path.removable_parent() {
        // Try to remove the parent directory as well.
        remove_empty_dirs(p, retries, delay)
    } else {
        Ok(())
    }
}

/// Removes the read-only and hidden attributes on a file.
#[cfg(windows)]
fn unset_attributes(path: &Path) -> io::Result<()> {
    let path = to_u16s(path);

    let attribs = unsafe { kernel32::GetFileAttributesW(path.as_ptr()) };

    if attribs == INVALID_FILE_ATTRIBUTES {
        return Err(io::Error::last_os_error());
    }

    let new_attribs = attribs &
        !(FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_HIDDEN);

    if attribs != new_attribs {
        let ret =
            unsafe { kernel32::SetFileAttributesW(path.as_ptr(), new_attribs) };
        if ret == 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

/// Wrapper for `fs::remove_file` to ignore the case where the file or path to
/// the file does not exist.
#[cfg(windows)]
fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) => {
            match err.kind() {
                // It's fine if the file already doesn't exist.
                io::ErrorKind::NotFound => Ok(()),
                io::ErrorKind::PermissionDenied => {
                    // Unset read-only and hidden attributes and try the copy
                    // again. Windows will fail to remove these types of files.
                    if let Err(err) = unset_attributes(path) {
                        Err(err)
                    } else {
                        // Try again, but only once. Don't want to get into an
                        // infinite loop.
                        fs::remove_file(path)
                    }
                }

                // Anything else is still an error.
                _ => Err(err),
            }
        }
        Ok(()) => {
            if path.is_file() {
                // The operating system lied to us. The file still exists.
                Err(io::Error::from_raw_os_error(
                        winerror::ERROR_FILE_EXISTS as i32
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(not(windows))]
pub fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) => {
            match err.kind() {
                // It's fine if the file already doesn't exist.
                io::ErrorKind::NotFound => Ok(()),

                // Anything else is still an error.
                _ => Err(err),
            }
        }
        Ok(()) => Ok(()),
    }
}

/// Removes a file with a retry. This can be useful on Windows if someone has a
/// lock on the file.
pub fn remove_file_retry(
    path: &Path,
    retries: usize,
    delay: Duration,
) -> io::Result<()> {
    match remove_file(path) {
        Err(err) => {
            if retries > 0 {
                thread::sleep(delay);
                remove_file_retry(path, retries - 1, delay * 2)
            } else {
                Err(err)
            }
        }
        Ok(()) => Ok(()),
    }
}


/// Wraps `fs::copy` to be able to fix 'hidden' and 'readonly' attributes on the
/// `to` path.
#[cfg(windows)]
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    match fs::copy(from, to) {
        Err(err) => {
            if err.kind() == io::ErrorKind::PermissionDenied {
                // Unset read-only and hidden attributes and try the copy
                // again. Windows will fail to copy over files with these
                // attributes set.
                if let Err(err) = unset_attributes(to) {
                    Err(err)
                } else {
                    // Try again.
                    fs::copy(from, to)
                }
            } else {
                Err(err)
            }
        }
        Ok(n) => Ok(n),
    }
}

#[cfg(unix)]
fn lstat(p: &Path) -> io::Result<stat64> {
    let p = ffi::CString::new(p.as_os_str().as_bytes())?;

    let mut stat: stat64 = unsafe { mem::zeroed() };

    let ret = unsafe { lstat64(p.as_ptr(), &mut stat as *mut stat64) };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(stat)
    }
}

#[cfg(unix)]
fn copy_timestamps(from: &Path, to: &Path) -> io::Result<()> {

    let to = ffi::CString::new(to.as_os_str().as_bytes())?;

    let stat = lstat(from)?;

    let times: [timespec; 2] = [
        timespec {
            tv_sec: stat.st_atime,
            tv_nsec: stat.st_atime_nsec,
        },
        timespec {
            tv_sec: stat.st_mtime,
            tv_nsec: stat.st_mtime_nsec,
        },
    ];

    let ret = unsafe {
        utimensat(AT_FDCWD, to.as_ptr(), &times as *const timespec, 0)
    };

    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    let n = fs::copy(from, to)?;

    copy_timestamps(from, to)?;

    Ok(n)
}

/// Copies a file with a retry. When copying files across the network, this can
/// be useful to work around transient failures.
pub fn copy_retry(
    from: &Path,
    to: &Path,
    retries: usize,
    delay: Duration,
) -> io::Result<u64> {
    match copy(from, to) {
        Err(err) => {
            match err.kind() {
                // These errors are not worth retrying as they almost never
                // succeed with a retry.
                io::ErrorKind::NotFound |
                io::ErrorKind::PermissionDenied => Err(err),

                // Anything else should have a retry.
                _ => {
                    if retries > 0 {
                        thread::sleep(delay);
                        copy_retry(from, to, retries - 1, delay * 2)
                    } else {
                        Err(err)
                    }
                }
            }
        }
        Ok(n) => Ok(n),
    }
}

/// Get metadata with a retry.
pub fn metadata_retry(
    path: &Path,
    retries: usize,
    delay: Duration,
) -> io::Result<fs::Metadata> {
    match fs::metadata(path) {
        Err(err) => {
            match err.kind() {
                // These errors are not worth retrying as they almost never
                // succeed with a retry.
                io::ErrorKind::NotFound |
                io::ErrorKind::PermissionDenied => Err(err),

                // Anything else should have a retry
                _ => {
                    if retries > 0 {
                        thread::sleep(delay);
                        metadata_retry(path, retries - 1, delay * 2)
                    } else {
                        Err(err)
                    }
                }
            }
        }
        Ok(m) => Ok(m),
    }
}

pub trait PathExt {
    /// Returns the parent of the given path if it can be removed. Returns None
    /// if the parent directory is a root or prefix component. These types of
    /// directories cannot be removed.
    fn removable_parent(&self) -> Option<&Path>;

    /// Returns a normalized path. This does not touch the file system at all.
    fn norm(&self) -> PathBuf;

    /// Returns `true` if the path is considered "sandboxed". That is, if its
    /// first path component is a `Normal` component. The path is expected to
    /// be normalized.
    fn is_sandboxed(&self) -> bool;

    /// Returns `true` if the path is empty.
    fn is_empty(&self) -> bool;
}

impl PathExt for Path {
    fn removable_parent(&self) -> Option<&Path> {
        let mut comps = self.components();
        comps.next_back().and_then(|p| match p {
            Component::Normal(_) => {
                let parent = comps.as_path();
                match comps.next_back() {
                    Some(Component::Normal(_)) => Some(parent),
                    _ => None,
                }
            }
            _ => None,
        })
    }

    fn norm(&self) -> PathBuf {
        let mut new_path = PathBuf::new();

        let mut components = self.components();

        if self.as_os_str().len() >= 260 {
            // If the path is >= 260 characters, we should prefix it with '\\?\'
            // if possible.
            if let Some(c) = components.next() {
                match c {
                    Component::CurDir => {}
                    Component::RootDir |
                    Component::ParentDir |
                    Component::Normal(_) => {
                        // Can't add the prefix. It's a relative path.
                        new_path.push(c.as_os_str());
                    }
                    Component::Prefix(prefix) => {
                        match prefix.kind() {
                            Prefix::UNC(server, share) => {
                                let mut p = ffi::OsString::from(r"\\?\UNC\");
                                p.push(server);
                                p.push(r"\");
                                p.push(share);
                                new_path.push(p);
                            }
                            Prefix::Disk(_) => {
                                let mut p = ffi::OsString::from(r"\\?\");
                                p.push(c.as_os_str());
                                new_path.push(p);
                            }
                            _ => {
                                new_path.push(c.as_os_str());
                            }
                        };
                    }
                };
            }
        }

        for c in components {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    let pop = match new_path.components().next_back() {
                        Some(Component::Prefix(_)) |
                        Some(Component::RootDir) => true,
                        Some(Component::Normal(s)) => !s.is_empty(),
                        _ => false,
                    };

                    if pop {
                        new_path.pop();
                    } else {
                        new_path.push("..");
                    }
                }
                _ => {
                    new_path.push(c.as_os_str());
                }
            };
        }

        if new_path.as_os_str().is_empty() {
            new_path.push(".");
        }

        new_path
    }

    fn is_sandboxed(&self) -> bool {
        if let Some(c) = self.components().next() {
            match c {
                Component::CurDir => true,
                Component::Normal(_) => true,
                _ => false,
            }
        } else {
            // Nothing in the path. It can be considered sandboxed.
            true
        }
    }

    fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox() {
        assert!(Path::new("foo").is_sandboxed());
        assert!(Path::new("./foo").is_sandboxed());
        assert!(Path::new(".").is_sandboxed());
        assert!(Path::new("").is_sandboxed());
        assert!(!Path::new("../foo").is_sandboxed());
        assert!(!Path::new("/foo").is_sandboxed());
    }

    #[test]
    fn test_parent_dir() {
        assert_eq!(Path::new("foo").removable_parent(), None);
        assert_eq!(Path::new("/foo").removable_parent(), None);
        assert_eq!(Path::new("/").removable_parent(), None);
        assert_eq!(
            Path::new("foo/bar").removable_parent(),
            Some(Path::new("foo"))
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_parent_dir_win() {
        assert_eq!(
            Path::new(r"C:\foo\bar").removable_parent(),
            Some(Path::new(r"C:\foo"))
        );
        assert_eq!(Path::new(r"C:\foo").removable_parent(), None);
        assert_eq!(Path::new(r"C:\").removable_parent(), None);
        assert_eq!(Path::new(r"\\?\C:\foo").removable_parent(), None);
        assert_eq!(Path::new(r"\\?\C:\").removable_parent(), None);
        assert_eq!(
            Path::new(r"\\?\C:\foo\bar").removable_parent(),
            Some(Path::new(r"\\?\C:\foo"))
        );
        assert_eq!(Path::new(r"\\server\share").removable_parent(), None);
        assert_eq!(Path::new(r"\\server\share\foo").removable_parent(), None);
        assert_eq!(
            Path::new(r"\\server\share\foo\bar").removable_parent(),
            Some(Path::new(r"\\server\share\foo"))
        );
        assert_eq!(Path::new(r"\\?\UNC\server\share").removable_parent(), None);
        assert_eq!(
            Path::new(r"\\?\UNC\server\share\foo").removable_parent(),
            None
        );
        assert_eq!(
            Path::new(r"\\?\UNC\server\share\foo\bar").removable_parent(),
            Some(Path::new(r"\\?\UNC\server\share\foo"))
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_norm() {
        assert_eq!(Path::new("../foo").parent(), Some(Path::new("..")));
        assert_eq!(Path::new("foo").norm(), Path::new("foo"));
        assert_eq!(Path::new("./foo").norm(), Path::new("foo"));
        assert_eq!(Path::new(".").norm(), Path::new("."));
        assert_eq!(Path::new("..").norm(), Path::new(".."));
        assert_eq!(Path::new(r"..\..").norm(), Path::new(r"..\.."));
        assert_eq!(Path::new(r"..\..\..").norm(), Path::new(r"..\..\.."));
        assert_eq!(Path::new("").norm(), Path::new("."));
        assert_eq!(Path::new("foo/bar").norm(), Path::new(r"foo\bar"));
        assert_eq!(Path::new("C:/foo/../bar").norm(), Path::new(r"C:\bar"));
        assert_eq!(Path::new("C:/../bar").norm(), Path::new(r"C:\bar"));
        assert_eq!(Path::new("C:/../../bar").norm(), Path::new(r"C:\bar"));
        assert_eq!(Path::new("foo//bar///").norm(), Path::new(r"foo\bar"));
        assert_eq!(
            Path::new(r"\\server\share\..\foo").norm(),
            Path::new(r"\\server\share\foo")
        );
        assert_eq!(
            Path::new(r"\\server\share\..\foo\..").norm(),
            Path::new(r"\\server\share")
        );
        assert_eq!(
            Path::new(r"..\foo\..\..\bar").norm(),
            Path::new(r"..\..\bar")
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_norm() {
        assert_eq!(Path::new("../foo").parent(), Some(Path::new("..")));
        assert_eq!(Path::new("foo").norm(), Path::new("foo"));
        assert_eq!(Path::new("./foo").norm(), Path::new("foo"));
        assert_eq!(Path::new(".").norm(), Path::new("."));
        assert_eq!(Path::new("..").norm(), Path::new(".."));
        assert_eq!(Path::new("../..").norm(), Path::new("../.."));
        assert_eq!(Path::new("../../..").norm(), Path::new("../../.."));
        assert_eq!(Path::new("").norm(), Path::new("."));
        assert_eq!(Path::new("foo/bar").norm(), Path::new("foo/bar"));
        assert_eq!(Path::new("/foo/../bar").norm(), Path::new("/bar"));
        assert_eq!(Path::new("/../bar").norm(), Path::new("/bar"));
        assert_eq!(Path::new("/../../bar").norm(), Path::new("/bar"));
        assert_eq!(Path::new("foo//bar///").norm(), Path::new("foo/bar"));
        assert_eq!(
            Path::new("../foo/../../bar").norm(),
            Path::new("../../bar")
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_norm_long_paths() {
        use std::iter;

        let long_name: String = iter::repeat('a').take(260).collect();
        let long_name = long_name.as_str();

        // Long paths
        assert_eq!(
            PathBuf::from(String::from(r"C:\") + long_name).norm(),
            PathBuf::from(String::from(r"\\?\C:\") + long_name)
        );
        assert_eq!(
            PathBuf::from(String::from(r"\\server\share\") + long_name).norm(),
            PathBuf::from(String::from(r"\\?\UNC\server\share\") + long_name)
        );

        // Long relative paths
        assert_eq!(
            PathBuf::from(String::from(r"..\relative\") + long_name).norm(),
            PathBuf::from(String::from(r"..\relative\") + long_name)
        );
        assert_eq!(
            PathBuf::from(String::from(r".\relative\") + long_name).norm(),
            PathBuf::from(String::from(r"relative\") + long_name)
        );
    }
}
