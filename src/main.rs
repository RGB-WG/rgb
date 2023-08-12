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
extern crate serde_crate as serde;

mod command;
mod args;
mod resolver;

use std::process::ExitCode;

use bpw::{Config, Exec, LogLevel};
use clap::Parser;
use rgb::descriptor::RgbKeychain;
use rgb_rt::RuntimeError;

pub use crate::args::Args;
pub use crate::command::Command;
pub use crate::resolver::PanickingResolver;

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> Result<(), RuntimeError> {
    let mut args = Args::parse();
    args.process();
    LogLevel::from_verbosity_flag_count(args.verbose).apply();
    trace!("Command-line arguments: {:#?}", &args);

    eprintln!("RGB: command-line wallet for RGB smart contracts");
    eprintln!("     by LNP/BP Standards Association\n");

    let conf = Config::load(&args.conf_path("rgb"));
    debug!("Executing command: {}", args.command);
    args.exec::<RgbKeychain>(conf, "rgb")
}
