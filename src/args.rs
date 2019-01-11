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

use clap::{App, AppSettings, Arg, ArgMatches};

#[derive(Debug)]
pub struct Args {
    pub dryrun: bool,
    pub force: bool,
    pub verify_copy: bool,
    pub sandbox_src: bool,
    pub sandbox_dest: bool,
    pub threads: usize,
    pub retries: usize,
    pub dest: PathBuf,
    pub manifest: PathBuf,
    pub program: String,
    pub args: Vec<String>,
}

impl Args {
    #[rustfmt::skip]
    pub fn parse() -> Self {
        let matches = App::new("ubercopy")
            .version(clap::crate_version!())
            .author(clap::crate_authors!())
            .about(clap::crate_description!())
            .setting(AppSettings::TrailingVarArg)
            .args(&[
                Arg::with_name("dryrun")
                    .help("Don't actually do anything. Just print what might \
                          happen.")
                    .long("dryrun")
                    .alias("dry-run")
                    .short("n"),

                Arg::with_name("force")
                    .help("Don't be smart about anything. Copy everything \
                          regardless.")
                    .long("force")
                    .short("f"),

                Arg::with_name("verify-copy")
                    .help("After copying, verify that all files match.")
                    .long("verify-copy"),

                Arg::with_name("sandbox-src")
                    .help("Don't allow source paths to escape the current \
                          directory.")
                    .long("sandbox-src"),

                Arg::with_name("sandbox-dest")
                    .help("Don't allow destination paths to escape the current \
                          directory.")
                    .long("sandbox-dest"),

                Arg::with_name("sandbox")
                    .help("Implies both --sandbox-src and --sandbox-dest.")
                    .long("sandbox"),

                Arg::with_name("threads")
                    .help("Number of threads to use for copying.")
                    .takes_value(true)
                    .long("threads")
                    .short("t")
                    .default_value("20"),

                Arg::with_name("retries")
                    .help("Number of times to retry a copy or deletion before \
                          giving up.")
                    .long("retries")
                    .short("r")
                    .takes_value(true)
                    .default_value("5"),

                Arg::with_name("dest")
                    .help("Makes all destination paths relative to this path.")
                    .long("dest")
                    .takes_value(true),

                Arg::with_name("manifest")
                    .help("Path to the manifest to generate.")
                    .index(1)
                    .required(true),

                Arg::with_name("program")
                    .help("Generator program name.")
                    .index(2)
                    .required(true),

                Arg::with_name("args")
                    .help("Generator program arguments.")
                    .min_values(1),
            ])
            .get_matches();

        Args::parse_matches(&matches)
    }

    fn parse_matches<'a>(matches: &ArgMatches<'a>) -> Self {
        Args {
            dryrun: matches.is_present("dryrun"),
            force: matches.is_present("force"),
            verify_copy: matches.is_present("verify-copy"),
            sandbox_src: matches.is_present("sandbox")
                || matches.is_present("sandbox-src"),
            sandbox_dest: matches.is_present("sandbox")
                || matches.is_present("sandbox-dest"),
            threads: clap::value_t!(matches, "threads", usize)
                .unwrap_or_else(|e| e.exit()),
            retries: clap::value_t!(matches, "retries", usize)
                .unwrap_or_else(|e| e.exit()),
            dest: matches
                .value_of("dest")
                .map_or(PathBuf::from(""), PathBuf::from),
            manifest: PathBuf::from(matches.value_of("manifest").unwrap()),
            program: matches.value_of("program").unwrap().to_string(),
            args: match matches.values_of("args") {
                None => vec![],
                Some(vals) => vals.map(String::from).collect(),
            },
        }
    }
}
