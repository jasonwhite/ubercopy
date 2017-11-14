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

pub const OVERLAP: &'static str = "\
Error: Some file(s) are both sources and destinations. This is a race
       condition. The files listed above are listed in the manifest as both
       sources and destinations.";

pub const DUPLICATES: &'static str = "\
Error: Duplicate destination path(s). The files listed above appear more than
       once as destinations in the manifest. This is a race condition.";

pub const MISSING_SOURCES: &'static str = "\
Error: The source file(s) listed above are either missing or have some other
       problem. Make sure these files exist and are accessible.";

pub const CREATE_DIRS: &'static str = "\
Error: The destination directories listed above failed to get created.";

pub const DELETE: &'static str = "\
Error: The above destination files failed to get deleted.";

pub const DELETE_DIRS: &'static str = "\
Error: The above destination directories failed to get deleted.";

pub const COPIES: &'static str = "\
Error: The copy operations listed above failed.";

pub const SANITY_NOT_COPIED: &'static str = "\
Error: The copy operation(s) listed above are still incomplete even after
       copying them. This can happen if a file was modified by another process
       during the copy. Simply re-running the copy usually fixes it. This sanity
       check can be skipped with the `--skip-sanity` flag.";

pub const SANITY_ERRORS: &'static str = "\
Error: Post-copy error. The source file(s) listed above are either missing or
       have some other problem. This can happen if a source file was removed or
       changed somehow just after it was copied to the destination. This
       indicates a race condition with some other process. Make sure nothing
       else is messing with these files during the copy. This sanity check can
       be skipped with the `--skip-sanity` flag.";
