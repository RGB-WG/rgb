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

use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::ByteArray;
use bpwallet::fs::FsTextStore;
use bpwallet::psbt::TxParams;
use bpwallet::{Network, Sats, Vout, Wpkh, XpubDerivable};
use clap::ValueHint;
use hypersonic::{AuthToken, ContractId};
use rgb::popls::bp::file::{DirBarrow, DirMound};
use rgb::popls::bp::ConstructParams;
use rgb::{CreateParams, Outpoint, SealType};
use rgbp::descriptor::{Opret, Tapret};
use rgbp::wallet::file::DirRuntime;
use rgbp::wallet::{OpretWallet, TapretWallet};
use strict_encoding::StrictSerialize;

pub const RGB_WALLET_ENV: &str = "RGB_WALLET";
pub const RGB_SEAL_ENV: &str = "RGB_SEAL";
pub const RGB_NETWORK_ENV: &str = "RGB_NETWORK";

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

    /// Initialize data directory if it doesn't exit
    #[clap(long, global = true)]
    pub init: bool,

    /// Type of single-use seals to use
    #[clap(short, long, global = true, default_value = "bctr", env = RGB_SEAL_ENV)]
    pub seal: SealType,

    /// Network to use
    #[arg(short, long, global = true, default_value = "testnet4", env = RGB_NETWORK_ENV)]
    pub network: Network,

    /// Command to execute
    #[clap(subcommand)]
    pub command: Cmd,
}

#[derive(Parser)]
pub enum Cmd {
    /// Issue a new RGB contract
    Issue {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Parameters and data for the contract
        #[clap(value_hint = ValueHint::FilePath)]
        params: Option<PathBuf>,
    },

    /// Import contract articles
    Import {
        /// Contract articles to process
        #[clap(value_hint = ValueHint::FilePath)]
        articles: PathBuf,
    },

    /// Export contract articles
    Export {
        /// Contract id to export
        contract: ContractId,

        /// Path to export articles to
        #[clap(value_hint = ValueHint::FilePath)]
        file: Option<PathBuf>,
    },

    Backup {
        /// Path for saving backup tar file
        #[clap(default_value = "rgb-backup.tar", value_hint = ValueHint::FilePath)]
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

    /// List known wallets
    Wallets,

    /// Create a new wallet
    Create {
        /// Wallet name
        name: String,

        /// Extended pubkey descriptor
        descriptor: String,
    },

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

    /// Execute a script, producing prefabricated operation bundle and PSBT
    #[clap(alias = "x")]
    Exec {
        /// Print PSBT to STDOUT
        #[clap(short, long, global = true)]
        print: bool,

        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// YAML file with a script to execute
        #[clap(value_hint = ValueHint::FilePath)]
        script: PathBuf,

        /// File to save the produced prefabricated operation bundle
        #[clap(value_hint = ValueHint::FilePath)]
        bundle: PathBuf,

        /// Fees for PSBT
        fee: Sats,

        /// File to save the produced PSBT
        ///
        /// If not provided, uses the same filename as for the bundle, replacing the extension with
        /// 'psbt'.
        #[clap(value_hint = ValueHint::FilePath)]
        psbt: Option<PathBuf>,
    },

    /// Create a consignment transferring part of a contract state to another peer
    #[clap(alias = "c")]
    Consign {
        /// Contract to use for the consignment
        contract: ContractId,

        /// List of tokens of authority which should serve as a contract terminals
        #[clap(short, long)]
        terminals: Vec<AuthToken>,

        /// Location to save the consignment file to
        #[clap(value_hint = ValueHint::FilePath)]
        output: PathBuf,
    },

    /// Verify and accept a consignment
    #[clap(alias = "a")]
    Accept {
        /// File with consignment to accept
        #[clap(value_hint = ValueHint::FilePath)]
        input: PathBuf,
    },
}

impl Args {
    pub fn mound(&self) -> DirMound {
        if self.init {
            let _ = fs::create_dir_all(&self.data_dir.join(SealType::BitcoinOpret.to_string()));
            let _ = fs::create_dir_all(&self.data_dir.join(SealType::BitcoinTapret.to_string()));
        }
        DirMound::load(&self.data_dir)
    }

    fn wallet_file(&self, name: Option<&str>) -> PathBuf {
        let mut path = self.data_dir.join(self.seal.to_string());
        path.join(name.unwrap_or("default"))
    }

    pub fn wallet_provider(&self, name: Option<&str>) -> FsTextStore {
        FsTextStore::new(self.wallet_file(name)).expect("broken directory structure")
    }

    pub fn runtime(&self, name: Option<&str>) -> DirRuntime {
        let provider = self.wallet_provider(name);
        let wallet = match self.seal {
            SealType::BitcoinOpret => {
                let wallet = OpretWallet::load(provider, true).unwrap_or_else(|_| {
                    panic!(
                        "Error: unable to load opret wallet from path `{}`",
                        self.wallet_file(name).display()
                    )
                });
                DirBarrow::with_opret(self.seal, self.mound(), wallet)
            }
            SealType::BitcoinTapret => {
                let wallet = TapretWallet::load(provider, true).unwrap_or_else(|_| {
                    panic!(
                        "Error: unable to load tapret wallet from path `{}`",
                        self.wallet_file(name).display()
                    )
                });
                DirBarrow::with_tapret(self.seal, self.mound(), wallet)
            }
        };
        // TODO: Sync wallet if needed
        wallet.into()
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        match &self.command {
            Cmd::Issue {
                params: None,
                wallet: _,
            } => {
                println!(
                    "To issue a new contract please specify a parameters file. A contract may be \
                     issued under one of the codex listed below."
                );
                println!();
                println!("{:<32}\t{:<64}\tDeveloper", "Name", "ID");
                for (codex_id, schema) in self.mound().schemata() {
                    println!("{:<32}\t{codex_id}\t{}", schema.codex.name, schema.codex.developer);
                }
            }
            Cmd::Issue {
                params: Some(params),
                wallet,
            } => {
                let mut runtime = self.runtime(wallet.as_deref());
                let file = File::open(params).expect("Unable to open parameters file");
                let params = serde_yaml::from_reader::<_, CreateParams<Outpoint>>(file)?;
                let contract_id = runtime.issue_to_file(params);
                println!("A new contract issued with ID {contract_id}");
            }

            //Cmd::Import { articles } => self.mound().import_file(articles),
            //Cmd::Export { contract, file } => self.mound().export_file(contract, file),
            Cmd::Create { name, descriptor } => {
                let provider = self.wallet_provider(Some(name));
                let xpub = XpubDerivable::from_str(descriptor).expect("Invalid extended pubkey");
                let noise = xpub.xpub().chain_code().to_byte_array();
                match self.seal {
                    SealType::BitcoinOpret => {
                        OpretWallet::create(
                            provider,
                            Opret::new_unfunded(Wpkh::from(xpub), noise),
                            self.network,
                            true,
                        )
                        .expect("unable to create wallet");
                    }
                    SealType::BitcoinTapret => {
                        TapretWallet::create(
                            provider,
                            Tapret::key_only_unfunded(xpub, noise),
                            self.network,
                            true,
                        )
                        .expect("unable to create wallet");
                    }
                }
            }

            Cmd::Contracts => {
                let mound = self.mound();
                for contract_id in mound.contract_ids() {
                    println!("{contract_id}");
                }
            }

            Cmd::State {
                wallet,
                all,
                contract,
            } => {
                for (contract_id, state) in self.runtime(wallet.as_deref()).state(*contract) {
                    println!("---");
                    println!("Contract ID: {contract_id}");
                    println!("---");
                    let state = serde_yaml::to_string(state).expect("unable to generate YAML");
                    println!("{state}");
                    println!();
                }
            }

            Cmd::Exec {
                wallet,
                script,
                fee,
                bundle: bundle_filename,
                psbt: psbt_filename,
                print,
            } => {
                let mut runtime = self.runtime(wallet.as_deref());
                let src = File::open(script).expect("unable to open script file");
                let items = serde_yaml::from_reader::<_, Vec<ConstructParams>>(src)?;
                let bundle = runtime.bundle(items);
                assert!(
                    bundle.defines().all(|vout| vout == Vout::from(0)),
                    "currently only a single self-seal is supported, which must be a first output"
                );
                bundle
                    .strict_serialize_to_file::<{ usize::MAX }>(&bundle_filename)
                    .expect("unable to write output file");

                let (psbt, _) = runtime
                    .construct_psbt(&bundle, TxParams::with(*fee))
                    .expect("unable to construct PSBT");
                let mut psbt_file = File::create_new(
                    psbt_filename
                        .as_ref()
                        .unwrap_or(bundle_filename)
                        .with_extension("psbt"),
                )
                .expect("unable to create PSBT");
                psbt.encode(psbt.version, &mut psbt_file)
                    .expect("unable to write PSBT");
                if *print {
                    println!("{psbt}");
                }
            }

            Cmd::Consign {
                contract,
                terminals,
                output,
            } => {
                let mut mound = self.mound();
                match self.seal {
                    SealType::BitcoinOpret => {
                        mound.bc_opret.consign_to_file(*contract, terminals, output)
                    }
                    SealType::BitcoinTapret => mound
                        .bc_tapret
                        .consign_to_file(*contract, terminals, output),
                }
                .expect("unable to consign contract");
            }

            Cmd::Accept { input } => {
                let mut mound = self.mound();
                match self.seal {
                    SealType::BitcoinOpret => mound.bc_opret.consume_from_file(input),
                    SealType::BitcoinTapret => mound.bc_opret.consume_from_file(input),
                }
                .unwrap_or_else(|err| panic!("Unable to accept a consignment: {err}"));
            }

            _ => todo!(),
        }
        Ok(())
    }
}
