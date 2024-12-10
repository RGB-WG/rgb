// Standard Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
// Copyright (C) 2025 RGB Consortium, Switzerland.
// Copyright (C) 2019-2025 Dr Maxim Orlovsky.
// All rights under the above copyrights are reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use std::path::PathBuf;

use bpwallet::fs::FsTextStore;
use clap::ValueHint;
use hypersonic::{AuthToken, CodexId, ContractId};
use rgb::popls::bp::file::{DirBarrow, DirMound};
use rgb::SealType;
use rgbp::wallet::file::DirRuntime;
use rgbp::wallet::{OpretWallet, TapretWallet};

pub const RGB_WALLET_ENV: &str = "RGB_WALLET";
pub const RGB_SEAL_ENV: &str = "RGB_SEAL";

pub const RGB_DATA_DIR_ENV: &str = "RGB_DATA_DIR";
#[cfg(target_os = "linux")]
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

#[derive(Parser)]
pub struct Args {
    /// Location of the data directory
    #[clap(
        short,
        long,
        global = true,
        default_value = RGB_DATA_DIR,
        env = RGB_DATA_DIR_ENV,
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// Type of single-use seals to use
    #[clap(short, long, global = true, default_value = "bctr", env = "RGB_SEAL_ENV")]
    pub seal: SealType,

    /// Command to execute
    #[clap(subcommand)]
    pub command: Cmd,
}

#[derive(Parser)]
pub enum Cmd {
    /// Issue a new RGB contract
    Issue {
        /// Codex used to issue the contract
        #[clap(requires = "params")]
        codex: Option<CodexId>,

        /// Parameters and data for the contract
        params: Option<PathBuf>,
    },

    /// Import contract articles
    Import {
        /// Contract articles to process
        articles: PathBuf,
    },

    /// Export contract articles
    Export {
        /// Contract id to export
        contract: ContractId,

        /// Path to export articles to
        file: Option<PathBuf>,
    },

    Backup {
        /// Path for saving backup tar file
        #[clap(default_value = "rgb-backup.tar")]
        file: PathBuf,
    },

    /// List contracts
    Contracts,

    /// Remove contract
    Purge {
        /// Force removal of a contract with a known state
        #[clap(short, long)]
        force: bool,

        /// Contract id to remove
        contract: ContractId,
    },

    /// Create a new wallet
    Create { name: String, descriptor: String },

    /// Print out a contract state
    #[clap(alias = "s")]
    State {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Present all the state, not just the one owned by the wallet
        #[clap(short, long, global = true)]
        all: bool,

        /// Print out just a single contract state
        contract: Option<ContractId>,
    },

    /// Execute a script
    #[clap(alias = "x")]
    Exec {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// YAML file with a script to execute
        script: PathBuf,
    },

    /// Create a consignment transferring part of a contract state to another peer
    #[clap(alias = "c")]
    Consign {
        /// List of tokens of authority which should serve as a contract terminals
        #[clap(short, long)]
        terminals: Vec<AuthToken>,

        /// Location to save the consignment file to
        output: PathBuf,
    },

    /// Verify and accept a consignment
    #[clap(alias = "a")]
    Accept {
        /// File with consignment to accept
        input: PathBuf,
    },
}

impl Args {
    pub fn mound(&self) -> DirMound { DirMound::load(&self.data_dir) }

    pub fn runtime(&self) -> DirRuntime {
        let provider = FsTextStore::new(self.data_dir.join(self.seal.to_string()))
            .expect("broken directory structure");
        match self.seal {
            SealType::BitcoinOpret => {
                let wallet = OpretWallet::load(provider, true).expect("unable to load the wallet");
                DirBarrow::with_opret(self.seal, self.mound(), wallet)
            }
            SealType::BitcoinTapret => {
                let wallet = TapretWallet::load(provider, true).expect("unable to load the wallet");
                DirBarrow::with_tapret(self.seal, self.mound(), wallet)
            }
            _ => panic!("unsupported wallet type"),
        }
        .into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        match &self.command {
            Cmd::Issue {
                codex: None,
                params: None,
            } => {
                println!("To issue a new contract please specify a codex ID and a parameters file");
                println!("Codex list:");
                println!("{:<32}\t{:<64}\tDeveloper", "Name", "ID");
                for (codex_id, schema) in self.mound().schemata() {
                    println!("{:<32}\t{codex_id}\t{}", schema.codex.name, schema.codex.developer);
                }
            }
            Cmd::Issue {
                codex: Some(codex_id),
                params: Some(params),
            } => {
                self.mound().issue_from_file(codex_id, params);
            }

            Cmd::Import { articles } => self.mound().import_file(articles),
            Cmd::Export { contract, file } => self.mound().export_file(contract, file),
            Cmd::Create { name, descriptor } => match self.seal {
                SealType::BitcoinOpret => {}
                SealType::BitcoinTapret => {}
                _ => panic!("unsupported seal type"),
            },

            Cmd::State {
                wallet: Some(name),
                all,
                contract,
            } => {
                for (contract_id, state) in self.runtime().state(all, contract) {
                    println!("---");
                    println!("Contract ID: {contract_id}");
                    println!("---");
                    println!("{state}");
                    println!();
                }
            }

            Cmd::Exec { wallet, script } => {}
            Cmd::Consign { .. } => todo!(),
            Cmd::Accept { .. } => todo!(),
        }
        Ok(())
    }
}
