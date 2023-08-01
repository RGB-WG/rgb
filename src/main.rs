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

use std::convert::Infallible;
use std::process::ExitCode;

use bp::{Tx, Txid};
use clap::Parser;
use rgb::resolvers::ResolveHeight;
use rgb::validation::{ResolveTx, TxResolverError};
use rgb::WitnessOrd;
use rgb_rt::RuntimeError;

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

pub const DEFAULT_ESPLORA: &str = "https://blockstream.info/testnet/api";

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> Result<(), RuntimeError> {
    let mut opts = Opts::parse();
    opts.process();
    LogLevel::from_verbosity_flag_count(opts.verbose).apply();
    trace!("Command-line arguments: {:#?}", &opts);

    #[derive(Default)]
    struct DumbResolver();
    impl ResolveHeight for DumbResolver {
        type Error = Infallible;
        fn resolve_height(&mut self, _: Txid) -> Result<WitnessOrd, Self::Error> {
            Ok(WitnessOrd::OffChain)
        }
    }
    impl ResolveTx for DumbResolver {
        fn resolve_tx(&self, _: Txid) -> Result<Tx, TxResolverError> { todo!() }
    }
    let mut resolver = DumbResolver::default();

    eprintln!("\nRGB: command-line wallet for RGB smart contracts");
    eprintln!("     by LNP/BP Standards Association\n");
    let mut runtime = opts.runtime()?;
    debug!("Executing command: {}", opts.command);
    opts.command.exec(&mut runtime, &mut resolver)?;
    Ok(())
}
