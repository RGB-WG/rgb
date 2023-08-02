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

use std::path::PathBuf;

use clap::ValueHint;
use rgb::Chain;
use rgb_rt::{DescriptorRgb, Runtime, RuntimeError};

use crate::{Command, RGB_DATA_DIR};

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct Opts {
    /// Set verbosity level.
    ///
    /// Can be used multiple times to increase verbosity.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(flatten)]
    pub config: Config,

    /// Command to execute.
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct Config {
    /// Data directory path.
    ///
    /// Path to the directory that contains RGB stored data.
    #[arg(
        short,
        long,
        global = true,
        default_value = RGB_DATA_DIR,
        env = "RGB_DATA_DIR",
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// Blockchain to use.
    #[arg(
        short = 'n',
        long,
        global = true,
        alias = "network",
        default_value = "testnet",
        env = "RGB_NETWORK"
    )]
    pub chain: Chain,
}

impl Opts {
    pub fn process(&mut self) {
        self.config.data_dir = PathBuf::from(
            shellexpand::tilde(&self.config.data_dir.display().to_string()).to_string(),
        );
    }

    pub fn runtime(&self) -> Result<Runtime, RuntimeError> {
        eprint!("Loading stock ... ");
        let runtime =
            Runtime::<DescriptorRgb>::load(self.config.data_dir.clone(), self.config.chain)?;
        eprintln!("success");

        Ok(runtime)
    }
}
