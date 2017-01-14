# Ubercopy

Ubercopy is a program for synchronizing the files as listed in a manifest. This
is similar to `rsync` or `robocopy`. However, neither `rsync` nor `robocopy`
accept a list of files with explicit destination paths. They will only
synchronize entire directories at a time.

Ubercopy works by delegating to another program to generate source and
destination file path pairs. Thus, a scripting language can be used to
orchestrate very complicated file copy operations. Where it would take multiple
invocations of `rsync` or `robocopy`, Ubercopy can do it in one go.

## An Example

Suppose we have a directory `src` with the following files:

    src
    |-- a.txt
    |-- b.txt
    `-- c.txt

We want to copy them to a directory called `dest`.

Lets create a file called `generate.py`:

```python
def add(src, dest):
    print('%s\t%s' % (src, dest))

add('src/a.txt', 'dest/a.txt')
add('src/b.txt', 'dest/b.txt')
add('src/c.txt', 'dest/c.txt')
```

When executed, we get the following output:

    $ python generate.py
    src/a.txt	dest/a.txt
    src/b.txt	dest/b.txt
    src/c.txt	dest/c.txt

This script is our *manifest generator*. That is, it generates a list of copy
operations.

To actually copy the files, we feed this script into `ubercopy`:

    $ ubercopy manifest -- python generate.py
    :: Creating destination directories...
      dest
    :: Updating modified files...
      "src\a.txt" -> "dest\a.txt"
      "src\b.txt" -> "dest\b.txt"
      "src\c.txt" -> "dest\c.txt"

Notice the file `manifest` was created:

    $ cat manifest
    src/a.txt	dest/a.txt
    src/b.txt	dest/b.txt
    src/c.txt	dest/c.txt

This is what `python generate.py` prints to standard output.

If run again immediately, without changing anything, nothing is done:

    $ ubercopy manifest -- python generate.py

Now lets modify `generate.py` by removing the copy of `src/c.txt` and run
`ubercopy` again:

```python
def add(src, dest):
    print('%s\t%s' % (src, dest))

add('src/a.txt', 'dest/a.txt')
add('src/b.txt', 'dest/b.txt')
#add('src/c.txt', 'dest/c.txt')
```

    $ ubercopy manifest -- python generate.py
    :: Deleting removed destinations...
      dest\c.txt

Ubercopy is able to determine that this file was removed from the generated
manifest by comparing it with the previously generated manifest. Thus, the
destination path gets deleted from disk. This is to ensure that incremental
copies are correct.

This is Ubercopy in a nutshell. See the `examples` directory for more examples.

## Parallel Copying

Copying files in parallel on a local hard drive may not lead to a significant
speedup. In fact, if too many threads are used, it may slow down file copying
due to the overhead of thread synchronization. However, copying files over a
network in parallel can lead to a significant speed up due to network
communication latencies. Thus, an option is provided to do file copying using
multiple threads:

    ubercopy manifest --threads 100 -- python generate.py

By default, 20 threads are used. Experiment with the number of threads to
achieve maximum network utilization.

## Building It

 1. Install [Rust][].

 3. Run `cargo build` in the root of the source directory.

[Rust]: https://www.rust-lang.org/en-US/install.html

## License

[MIT License](/LICENSE)

## Thanks

This tool was developed for internal use at [Environmental Systems Research
Institute](http://www.esri.com/) (Esri) who have graciously allowed me to retain
the copyright and publish it as open source software.
