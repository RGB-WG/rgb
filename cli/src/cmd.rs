// Wallet Library for RGB smart contracts
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
use std::process::exit;
use std::str::FromStr;

use amplify::ByteArray;
use bpwallet::cli::ResolverOpt;
use bpwallet::dbc::opret::OpretProof;
use bpwallet::dbc::tapret::TapretProof;
use bpwallet::fs::FsTextStore;
use bpwallet::indexers::esplora;
use bpwallet::psbt::TxParams;
use bpwallet::{AnyIndexer, Keychain, Network, Psbt, Sats, Vout, Wpkh, XpubDerivable};
use clap::ValueHint;
use rgb::popls::bp::file::{DirBarrow, DirMound};
use rgb::popls::bp::{PrefabBundle, PrefabParams};
use rgb::{AuthToken, ContractId, CreateParams, Outpoint, SealType};
use rgbp::descriptor::{Opret, Tapret};
use rgbp::wallet::file::DirRuntime;
use rgbp::wallet::{OpretWallet, TapretWallet};
use rgpsbt::RgbPsbt;
use strict_encoding::{StrictDeserialize, StrictSerialize};

use crate::opts::WalletOpts;

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

    /// Receiving a wallet address for gas funding
    Fund {
        /// Wallet to use
        #[clap(env = RGB_WALLET_ENV)]
        wallet: Option<String>,
    },

    /// Generate a new single-use seal
    Seal {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Nonce number to use
        nonce: u64,
    },

    /// Print out a contract state
    #[clap(alias = "s")]
    State {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Present all the state, not just the one owned by the wallet
        #[clap(short, long)]
        all: bool,

        /// Display global state entries
        #[clap(short, long, required_unless_present = "owned")]
        global: bool,

        /// Display owned state entries
        #[clap(short, long)]
        owned: bool,

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

    /// Complete finalizes PSBT and adds information about witness to the contracts mound
    Complete {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Prefabricated operation bundle, used in PSBT construction
        bundle: PathBuf,

        /// Signed PSBT
        psbt: PathBuf,
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
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// File with consignment to accept
        #[clap(value_hint = ValueHint::FilePath)]
        input: PathBuf,
    },
}

impl Args {
    pub fn mound(&self) -> DirMound {
        if self.init {
            let _ = fs::create_dir_all(self.data_dir.join(SealType::BitcoinOpret.to_string()));
            let _ = fs::create_dir_all(self.data_dir.join(SealType::BitcoinTapret.to_string()));
        }
        DirMound::load(&self.data_dir)
    }

    fn wallet_file(&self, name: Option<&str>) -> PathBuf {
        let path = self.data_dir.join(self.seal.to_string());
        path.join(name.unwrap_or("default"))
    }

    pub fn wallet_provider(&self, name: Option<&str>) -> FsTextStore {
        FsTextStore::new(self.wallet_file(name)).expect("Broken directory structure")
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

    pub fn indexer(&self, resolver: &ResolverOpt) -> AnyIndexer {
        let network = self.network.to_string();
        match (&resolver.esplora, &resolver.electrum, &resolver.mempool) {
            (None, Some(url), None) => AnyIndexer::Electrum(Box::new(
                electrum::Client::new(url).expect("Unable to initialize indexer"),
            )),
            (Some(url), None, None) => AnyIndexer::Esplora(Box::new(
                esplora::Client::new_esplora(&url.replace("{network}", &network))
                    .expect("Unable to initialize indexer"),
            )),
            (None, None, Some(url)) => AnyIndexer::Mempool(Box::new(
                esplora::Client::new_mempool(&url.replace("{network}", &network))
                    .expect("Unable to initialize indexer"),
            )),
            _ => {
                eprintln!(
                    "Error: no blockchain indexer specified; use either --esplora --mempool or \
                     --electrum argument"
                );
                exit(1);
            }
        }
    }

    pub fn exec(&self) -> anyhow::Result<()> {
        match &self.command {
            Cmd::Issue { params: None, wallet: _ } => {
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
            Cmd::Issue { params: Some(params), wallet } => {
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
                        .expect("Unable to create wallet");
                    }
                    SealType::BitcoinTapret => {
                        TapretWallet::create(
                            provider,
                            Tapret::key_only_unfunded(xpub, noise),
                            self.network,
                            true,
                        )
                        .expect("Unable to create wallet");
                    }
                }
            }

            Cmd::Contracts => {
                let mound = self.mound();
                for info in mound.contracts_info() {
                    println!("---");
                    println!("{}", serde_yaml::to_string(&info).expect("Unable to generate YAML"));
                }
            }

            Cmd::Fund { wallet } => {
                let mut runtime = self.runtime(wallet.as_deref());
                let addr = match self.seal {
                    SealType::BitcoinOpret => {
                        runtime.wallet_opret().next_address(Keychain::OUTER, true)
                    }
                    SealType::BitcoinTapret => {
                        runtime.wallet_tapret().next_address(Keychain::OUTER, true)
                    }
                };
                println!("{addr}");
            }

            Cmd::Seal { wallet, nonce } => {
                let mut runtime = self.runtime(wallet.as_deref());
                match runtime.auth_token(*nonce) {
                    None => println!("Wallet has no unspent outputs; try `fund` first"),
                    Some(token) => println!("{token}"),
                }
            }

            Cmd::State { wallet, all, global, owned, contract } => {
                let mut runtime = self.runtime(wallet.wallet.as_deref());
                if wallet.sync {
                    let indexer = self.indexer(&wallet.resolver);
                    match self.seal {
                        SealType::BitcoinOpret => runtime.wallet_opret().update(&indexer),
                        SealType::BitcoinTapret => runtime.wallet_tapret().update(&indexer),
                    };
                    println!();
                }
                let state =
                    if *all { runtime.state_all(*contract) } else { runtime.state(*contract) };
                for (contract_id, state) in state {
                    println!("{contract_id}");
                    if *global {
                        if state.immutable.is_empty() {
                            println!("global: # no known global state is defined by the contract");
                        } else {
                            println!(
                                "global: {:<16}\t{:<32}\t{:<32}\taddress",
                                "state name", "verified state", "unverified state"
                            );
                        }
                        for (name, map) in &state.immutable {
                            let mut first = true;
                            for (addr, atom) in map {
                                print!("\t{:<16}", if first { name.as_str() } else { " " });
                                print!("\t{:<32}", atom.verified.to_string());
                                if let Some(unverified) = &atom.unverified {
                                    print!("\t{unverified:<32}");
                                } else {
                                    print!("\t{:<32}", "~")
                                }
                                println!("\t{addr}");
                                first = false;
                            }
                        }

                        if state.computed.is_empty() {
                            println!(
                                "comp:   # no known computed state is defined by the contract"
                            );
                        } else {
                            print!(
                                "comp:   {:<16}\t{:<32}\t{:<32}\taddress",
                                "state name", "verified state", "unverified state"
                            );
                        }
                        for (name, val) in &state.computed {
                            println!("\t{name:<16}\t{val}");
                        }
                    }
                    if *owned {
                        if state.owned.is_empty() {
                            println!("owned:  # no known owned state is defined by the contract");
                        } else {
                            println!(
                                "owned:  {:<16}\t{:<32}\t{:<46}\toutpoint",
                                "state name", "value", "address"
                            );
                        }
                        for (name, map) in &state.owned {
                            let mut first = true;
                            for (addr, assignment) in map {
                                print!("\t{:<16}", if first { name.as_str() } else { " " });
                                print!("\t{:<32}", assignment.data.to_string());
                                print!("\t{addr:<46}");
                                println!("\t{}", assignment.seal);
                                first = false;
                            }
                        }
                    }
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
                let src = File::open(script).expect("Unable to open script file");
                let items = serde_yaml::from_reader::<_, Vec<PrefabParams>>(src)?;
                let bundle = runtime.bundle(items);
                assert!(
                    bundle.defines().all(|vout| vout == Vout::from(0)),
                    "currently only a single self-seal is supported, which must be a first output"
                );
                bundle
                    .strict_serialize_to_file::<{ usize::MAX }>(&bundle_filename)
                    .expect("Unable to write output file");

                let (mut psbt, _) = runtime
                    .construct_psbt(&bundle, TxParams::with(*fee))
                    .expect("Unable to construct PSBT");
                let mut psbt_file = File::create_new(
                    psbt_filename
                        .as_ref()
                        .unwrap_or(bundle_filename)
                        .with_extension("psbt"),
                )
                .expect("Unable to create PSBT");
                psbt.rgb_fill_csv(bundle)
                    .expect("Unable to embed RGB information to PSBT");
                psbt.encode(psbt.version, &mut psbt_file)
                    .expect("Unable to write PSBT");
                if *print {
                    println!("{psbt}");
                }
            }

            Cmd::Complete { wallet, bundle, psbt: psbt_file } => {
                let bundle = PrefabBundle::strict_deserialize_from_file::<{ usize::MAX }>(bundle)?;
                let mut psbt =
                    Psbt::decode(&mut File::open(psbt_file).expect("Unable to open PSBT"))?;
                psbt.rgb_complete()?;

                // TODO: Check that bundle matches PSBT

                let prevouts = psbt
                    .inputs()
                    .map(|inp| inp.previous_outpoint)
                    .collect::<Vec<_>>();

                let mut runtime = self.runtime(wallet.as_deref());
                // TODO: Attest possible oprets for tapret wallet
                match &mut runtime.0 {
                    DirBarrow::BcOpret(barrow) => {
                        let (mpc, dbc) = psbt.dbc_commit::<OpretProof>()?;
                        let tx = psbt.to_unsigned_tx();
                        barrow.attest(&bundle, &tx.into(), mpc, dbc, &prevouts);
                    }
                    DirBarrow::BcTapret(barrow) => {
                        let (mpc, dbc) = psbt.dbc_commit::<TapretProof>()?;
                        let tx = psbt.to_unsigned_tx();
                        barrow.attest(&bundle, &tx.into(), mpc, dbc, &prevouts);
                    }
                };

                psbt.encode(
                    psbt.version,
                    &mut File::create(psbt_file).expect("Unable to write PSBT"),
                )?;
            }

            Cmd::Consign { contract, terminals, output } => {
                let mut mound = self.mound();
                match self.seal {
                    SealType::BitcoinOpret => {
                        mound.bc_opret.consign_to_file(*contract, terminals, output)
                    }
                    SealType::BitcoinTapret => mound
                        .bc_tapret
                        .consign_to_file(*contract, terminals, output),
                }
                .expect("Unable to consign contract");
            }

            Cmd::Accept { wallet, input } => {
                let mut runtime = self.runtime(wallet.as_deref());
                runtime.consume_from_file(input)?;
            }

            _ => todo!(),
        }
        Ok(())
    }
}
