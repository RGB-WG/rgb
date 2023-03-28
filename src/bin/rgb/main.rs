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
extern crate strict_types;
#[macro_use]
extern crate log;
#[macro_use]
extern crate clap;

mod loglevel;
mod opts;
mod command;

use std::fs;

use clap::Parser;
use rgbfs::StockFs;
use rgbstd::persistence::Stock;

pub use crate::command::Command;
pub use crate::loglevel::LogLevel;
pub use crate::opts::Opts;

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

fn main() {
    let mut opts = Opts::parse();
    opts.process();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    trace!("Command-line arguments: {:#?}", &opts);

    let mut data_dir = opts.data_dir.clone();
    data_dir.push(opts.chain.to_string());
    fs::create_dir_all(&data_dir).unwrap();
    data_dir.push("stock.dat");
    let mut stock = Stock::load(&data_dir)
        .map_err(|_| warn!("stock file can't be read; re-creating"))
        .unwrap_or_default();

    let command = opts.command.unwrap_or_default();
    debug!("Executing command: {}", command);
    command.exec(&mut stock, opts.chain);

    stock.store(data_dir).expect("unable to save stock");
}
