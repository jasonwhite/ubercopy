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

use std::path::{Path, Component};
use std::fs;
use std::io;
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
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Convert a string to UTF-16.
#[cfg(windows)]
fn to_u16s<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
    let mut s : Vec<u16> = s.as_ref().encode_wide().collect();
    s.push(0);
    s
}

/// Returns the parent of the given path if it can be removed. Returns None if
/// the parent directory is a root or prefix component. These types of
/// directories cannot be removed.
pub fn removable_parent(path: &Path) -> Option<&Path> {
    let mut comps = path.components();
    comps.next_back().and_then(|p| {
        match p {
            Component::Normal(_) => {
                let parent = comps.as_path();
                match comps.next_back() {
                    Some(Component::Normal(_)) => Some(parent),
                    _ => None,
                }
            },
            _ => None,
        }
    })
}

/// Wrapper for `remove_dir` to ignore certain types of errors.
#[cfg(windows)]
pub fn remove_dir(path: &Path) -> io::Result<()> {
    match fs::remove_dir(path) {
        Err(err) => match err.raw_os_error().unwrap() as u32 {
            winerror::ERROR_FILE_NOT_FOUND => Ok(()),
            winerror::ERROR_DIR_NOT_EMPTY  => Ok(()),
            _   => Err(err),
        },
        Ok(()) => Ok(()),
    }
}

#[cfg(not(windows))]
pub fn remove_dir(path: &Path) -> io::Result<()> {
    fs::remove_dir(path)
}

/// Remove a directory with a retry.
pub fn remove_dir_retry(path: &Path, retries: usize, delay: Duration) -> io::Result<()> {
    match remove_dir(path) {
        Err(err) => {
            if retries > 0 {
                thread::sleep(delay);
                remove_dir_retry(path, retries-1, delay*2)
            }
            else {
                Err(err)
            }
        },
        Ok(()) => Ok(()),
    }
}

/// Deletes a directory and all its parent directories until it reaches a
/// directory that is not empty.
pub fn remove_empty_dirs(path: &Path, retries: usize, delay: Duration) -> io::Result<()> {
    try!(remove_dir_retry(path, retries, delay));

    if let Some(p) = removable_parent(path) {
        // Try to remove the parent directory as well.
        remove_empty_dirs(p, retries, delay)
    }
    else {
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

    let new_attribs = attribs & !(FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_HIDDEN);

    if attribs != new_attribs {
        let ret = unsafe { kernel32::SetFileAttributesW(path.as_ptr(), new_attribs) };
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
        Err(err) => match err.kind() {
            // It's fine if the file already doesn't exist.
            io::ErrorKind::NotFound => Ok(()),
            io::ErrorKind::PermissionDenied => {
                // Unset read-only and hidden attributes and try the copy again.
                // Windows will fail to remove these types of files.
                if let Err(err) = unset_attributes(path) {
                    Err(err)
                }
                else {
                    // Try again, but only once. Don't want to get into an
                    // infinite loop.
                    fs::remove_file(path)
                }
            },

            // Anything else is still an error.
            _ => Err(err),
        },
        Ok(()) => {
            if path.is_file() {
                // The operating system lied to us. The file still exists.
                Err(io::Error::from_raw_os_error(winerror::ERROR_FILE_EXISTS as i32))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(not(windows))]
pub fn remove_file(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(err) => match err.kind() {
            // It's fine if the file already doesn't exist.
            io::ErrorKind::NotFound => Ok(()),

            // Anything else is still an error.
            _ => Err(err),
        },
        Ok(()) => Ok(())
    }
}

/// Removes a file with a retry. This can be useful on Windows if someone has a
/// lock on the file.
pub fn remove_file_retry(path: &Path, retries: usize, delay: Duration) -> io::Result<()> {
    match remove_file(path) {
        Err(err) => {
            if retries > 0 {
                thread::sleep(delay);
                remove_file_retry(path, retries-1, delay*2)
            }
            else {
                Err(err)
            }
        },
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
                }
                else {
                    // Try again.
                    fs::copy(from, to)
                }
            }
            else {
                Err(err)
            }
        },
        Ok(n) => Ok(n),
    }
}

#[cfg(not(windows))]
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    fs::copy(from, to)
}

/// Copies a file with a retry. When copying files across the network, this can
/// be useful to work around transient failures.
pub fn copy_retry(from: &Path, to: &Path,
                  retries: usize, delay: Duration
                 ) -> io::Result<u64> {
    match copy(from, to) {
        Err(err) => match err.kind() {
            // These errors are not worth retrying as they almost never
            // succeed with a retry.
            io::ErrorKind::NotFound |
            io::ErrorKind::PermissionDenied => Err(err),

            // Anything else should have a retry.
            _ => {
                if retries > 0 {
                    thread::sleep(delay);
                    copy_retry(from, to, retries-1, delay*2)
                }
                else {
                    Err(err)
                }
            }
        },
        Ok(n) => Ok(n),
    }
}

/// Get metadata with a retry.
pub fn metadata_retry(path: &Path, retries: usize, delay: Duration) -> io::Result<fs::Metadata> {
    match fs::metadata(path) {
        Err(err) => match err.kind() {
            // These errors are not worth retrying as they almost never
            // succeed with a retry.
            io::ErrorKind::NotFound |
            io::ErrorKind::PermissionDenied => Err(err),

            // Anything else should have a retry
            _ => {
                if retries > 0 {
                    thread::sleep(delay);
                    metadata_retry(path, retries-1, delay*2)
                }
                else {
                    Err(err)
                }
            }
        },
        Ok(m) => Ok(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_dir2() {
        assert_eq!(removable_parent(&Path::new("foo")), None);
        assert_eq!(removable_parent(&Path::new("/foo")), None);
        assert_eq!(removable_parent(&Path::new("/")), None);
        assert_eq!(removable_parent(&Path::new("foo/bar")), Some(Path::new("foo")));
    }

    #[test]
    #[cfg(windows)]
    fn test_parent_dir2_win() {
        assert_eq!(removable_parent(&Path::new(r"C:\foo\bar")),
                   Some(Path::new(r"C:\foo")));
        assert_eq!(removable_parent(&Path::new(r"C:\foo")), None);
        assert_eq!(removable_parent(&Path::new(r"C:\")), None);
        assert_eq!(removable_parent(&Path::new(r"\\?\C:\foo")), None);
        assert_eq!(removable_parent(&Path::new(r"\\?\C:\")), None);
        assert_eq!(removable_parent(&Path::new(r"\\?\C:\foo\bar")),
                   Some(Path::new(r"\\?\C:\foo")));
        assert_eq!(removable_parent(&Path::new(r"\\server\share")), None);
        assert_eq!(removable_parent(&Path::new(r"\\server\share\foo")), None);
        assert_eq!(removable_parent(&Path::new(r"\\server\share\foo\bar")),
                   Some(Path::new(r"\\server\share\foo")));
        assert_eq!(removable_parent(&Path::new(r"\\?\UNC\server\share")), None);
        assert_eq!(removable_parent(&Path::new(r"\\?\UNC\server\share\foo")), None);
        assert_eq!(removable_parent(&Path::new(r"\\?\UNC\server\share\foo\bar")),
                   Some(Path::new(r"\\?\UNC\server\share\foo")));
    }
}
