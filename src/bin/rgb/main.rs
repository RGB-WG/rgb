// RGB smart contracts for Bitcoin & Lightning
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2019-2023 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2023 LNP/BP Standards Association. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod loglevel;

use std::path::PathBuf;
use clap::{Parser, ValueHint};
use rgbstd::Chain;

use self::loglevel::LogLevel;

#[cfg(any(target_os = "linux"))]
pub const RGB_DATA_DIR: &str = "~/.rgb";
#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
pub const RGB_DATA_DIR: &str = "~/.rgb";
#[cfg(target_os = "macos")]
pub const RGB_DATA_DIR: &str = "~/Library/Application Support/RGB Smart Contracts";
#[cfg(target_os = "windows")]
pub const RGB_DATA_DIR: &str = "~\\AppData\\Local\\RGB Smart Contracts";
#[cfg(target_os = "ios")]
pub const RGB_DATA_DIR: &str = "~/Documents";
#[cfg(target_os = "android")]
pub const RGB_DATA_DIR: &str = ".";

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Opts {
    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity.
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Data directory path.
    ///
    /// Path to the directory that contains RGB stored data.
    #[clap(
        short,
        long,
        global = true,
        default_value = RGB_DATA_DIR,
        env = "RGB_DATA_DIR",
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,


    /// Blockchain to use.
    #[clap(
        short = 'n',
        long,
        global = true,
        alias = "network",
        default_value = "signet",
        env = "RGB_NETWORK"
    )]
    pub chain: Chain,

    /// Command to execute.
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
pub enum Command {
    Noop
}

impl Opts {
    pub fn process(&mut self) {
        self.data_dir = PathBuf::from(shellexpand::tilde(&self.data_dir.display().to_string()).to_string());
    }
}

fn main() {
    let mut opts = Opts::parse();
    opts.process();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    trace!("Command-line arguments: {:#?}", &opts);

    debug!("Executing command: {}", opts.command);
}
