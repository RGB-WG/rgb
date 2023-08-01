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

use bp::XpubDescriptor;
use clap::ValueHint;
use rgb::Chain;
use rgb_rt::{DescriptorRgb, Runtime, RuntimeError, TapretKey};

use crate::{Command, DEFAULT_ESPLORA, RGB_DATA_DIR};

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
        default_value = "testnet",
        env = "RGB_NETWORK"
    )]
    pub chain: Chain,

    /// Path to wallet directory.
    #[clap(
        short,
        long,
        global = true,
        value_hint = ValueHint::DirPath,
        conflicts_with = "tr_key_only",
    )]
    pub wallet_path: Option<PathBuf>,

    /// Use tr(KEY) descriptor as wallet.
    #[clap(long, global = true)]
    pub tr_key_only: Option<XpubDescriptor>,

    /// Esplora server to use.
    #[clap(
        short,
        long,
        global = true,
        default_value = DEFAULT_ESPLORA,
        env = "RGB_ESPLORA_SERVER"
    )]
    pub esplora: String,

    #[clap(long, global = true)]
    pub sync: bool,

    /// Command to execute.
    #[clap(subcommand)]
    pub command: Command,
}

impl Opts {
    pub fn process(&mut self) {
        self.data_dir =
            PathBuf::from(shellexpand::tilde(&self.data_dir.display().to_string()).to_string());
    }

    pub fn runtime(&self) -> Result<Runtime, RuntimeError> {
        eprint!("Loading stock ... ");
        let mut runtime = Runtime::<DescriptorRgb>::load(self.data_dir.clone(), self.chain)?;
        eprint!("success");

        eprint!("Loading descriptor");
        let wallet = if let Some(d) = self.tr_key_only.clone() {
            eprint!(" from command-line argument ...");
            Ok(Some(bp_rt::Runtime::new(TapretKey::new_unfunded(d).into(), self.chain)))
        } else if let Some(wallet_path) = self.wallet_path.clone() {
            eprint!(" from specified wallet directory ...");
            bp_rt::Runtime::load(wallet_path).map(Some)
        } else {
            eprint!(" from wallet ...");
            let mut data_dir = self.data_dir.clone();
            data_dir.push(self.chain.to_string());
            bp_rt::Runtime::load(data_dir).map(Some)
        }?;
        if let Some(wallet) = wallet {
            runtime.attach(wallet)
        }
        eprintln!(" success");

        if self.sync || self.tr_key_only.is_some() {
            if let Some(wallet) = runtime.wallet_mut() {
                eprint!("Syncing ...");
                let indexer = esplora::Builder::new(&self.esplora)
                    .build_blocking()
                    .map_err(|err| RuntimeError::Custom(err.to_string()))?;
                if let Err(errors) = wallet.sync(&indexer) {
                    eprintln!(" partial, some requests has failed:");
                    for err in errors {
                        eprintln!("- {err}");
                    }
                } else {
                    eprintln!(" success");
                }
            }
        }

        Ok(runtime)
    }
}
