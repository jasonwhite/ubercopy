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
#[macro_use]
extern crate clap;

use duct;
use log;
use log4rs;

mod args;
mod copyop;
mod error;
mod iter;
mod manifest;
mod sync;
mod util;

use crate::args::Args;
use crate::manifest::Manifest;
use crate::sync::sync;

use std::env;
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use std::time::Duration;

use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

fn generate_manifest<T, P>(program: T, args: &[String], path: P)
where
    T: AsRef<str>,
    P: AsRef<Path>,
{
    log::info!("Creating manifest {:?}", path.as_ref());

    // Open the manifest
    let f = fs::File::create(path);
    if let Err(err) = f {
        log::error!("Failed to create manifest ({})", err);
        exit(1);
    }

    log::info!(
        "Running process `{}` with arguments {:?} to generate manifest",
        program.as_ref(),
        args
    );

    // Run the command to produce the manifest.
    let output = duct::cmd(program.as_ref(), args)
        .stdout_handle(f.unwrap())
        .run();

    if let Err(err) = output {
        log::error!("Failed to generate manifest. {}", err);
        exit(1);
    }
}

fn main() {
    let log_level = match env::var("UBERCOPY_LOG") {
        Ok(val) => log::LevelFilter::from_str(val.as_str())
            .unwrap_or(log::LevelFilter::Info),
        Err(_) => log::LevelFilter::Info,
    };

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} {l} {t} - {m}{n}",
        )))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(log_level))
        .unwrap();

    log4rs::init_config(config).unwrap();

    let args = Args::parse();

    let path_prev = &args.manifest;
    let mut path_next = args.manifest.as_os_str().to_os_string();
    path_next.push(".next");
    let path_next = Path::new(&path_next);

    generate_manifest(args.program, &args.args, &path_next);

    // Previous manifest
    let prev = match fs::File::open(path_prev) {
        Ok(f) => Manifest::parse_reader(
            BufReader::new(f),
            &args.dest,
            args.sandbox_src,
            args.sandbox_dest,
        ),
        Err(_) => Ok(Manifest::new()),
    };

    if let Err(err) = prev {
        println!("Error: Failed to parse manifest: {}", err);
        exit(1);
    }

    // Next manifest
    let next = Manifest::parse(
        &path_next,
        &args.dest.as_path(),
        args.sandbox_src,
        args.sandbox_dest,
    );

    if let Err(err) = next {
        println!("Error: Failed to parse manifest: {}", err);
        exit(1);
    }

    // Do the synchronization and handle errors.
    match sync(
        &prev.unwrap(),
        &next.unwrap(),
        args.dryrun,
        args.force,
        args.verify_copy,
        args.threads,
        args.retries,
        Duration::from_secs(1),
    ) {
        Ok(copied) => {
            println!("Successfully copied {} file(s).", copied);
        }
        Err(err) => {
            println!("{}", err);
            exit(1);
        }
    };

    if !args.dryrun {
        // Replace previous manifest with next manifest if everything succeeds.
        // This is an atomic way of saying that everything succeeded.
        if let Err(err) = fs::rename(&path_next, path_prev) {
            println!(
                "Failed to rename {:?} to {:?}: {}",
                path_next, path_prev, err
            );
            exit(1);
        }
    }
}
