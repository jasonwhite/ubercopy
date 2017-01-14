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

use clap::{App, AppSettings, Arg, ArgMatches};

#[derive(Debug)]
pub struct Args {
    pub dryrun: bool,
    pub force: bool,
    pub skip_sanity: bool,
    pub threads: usize,
    pub retries: usize,
    pub manifest: String,
    pub command: String,
    pub args: Vec<String>,
}

impl Args {

    pub fn parse() -> Self {
        let matches = App::new("ubercopy")
            .version(crate_version!())
            .author(crate_authors!())
            .about(crate_description!())
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

                Arg::with_name("skip-sanity")
                    .help("Skip doing a sanity check after copying all the files.")
                    .long("skip-sanity")
                    .short("S"),

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

                Arg::with_name("manifest")
                    .help("Path to the manifest to generate.")
                    .index(1)
                    .required(true),

                Arg::with_name("command")
                    .help("Generator command name.")
                    .index(2)
                    .required(true),

                Arg::with_name("args")
                    .help("Generator command arguments.")
                    .min_values(1),
            ])
            .get_matches();

        Args::parse_matches(&matches)
    }

    fn parse_matches<'a>(matches: &ArgMatches<'a>) -> Self {
        Args {
            dryrun: matches.is_present("dryrun"),
            force: matches.is_present("force"),
            skip_sanity: matches.is_present("skip-sanity"),
            threads: value_t!(matches, "threads", usize).unwrap_or_else(|e| e.exit()),
            retries: value_t!(matches, "retries", usize).unwrap_or_else(|e| e.exit()),
            manifest: matches.value_of("manifest").unwrap().to_string(),
            command: matches.value_of("command").unwrap().to_string(),
            args: match matches.values_of("args") {
                None => vec![],
                Some(vals) => vals.map(|s| String::from(s)).collect(),
            },
        }
    }
}

