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
use bpwallet::fs::FsTextStore;
use bpwallet::indexers::esplora;
use bpwallet::psbt::TxParams;
use bpwallet::{AnyIndexer, Keychain, Network, Psbt, Sats, Wpkh, XpubDerivable};
use clap::ValueHint;
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::file::{BpDirMound, DirBarrow};
use rgb::popls::bp::{OpRequestSet, PrefabBundle, WoutAssignment};
use rgb::{
    AuthToken, CallScope, Consensus, ContractId, ContractRef, CreateParams, MethodName, Outpoint,
    StateName,
};
use rgbp::descriptor::RgbDescr;
use rgbp::{CoinselectStrategy, RgbDirRuntime, RgbWallet};
use strict_encoding::{StrictDeserialize, StrictSerialize, TypeName};
use strict_types::StrictVal;

use crate::opts::WalletOpts;

pub const RGB_NETWORK_ENV: &str = "RGB_NETWORK";
pub const RGB_NO_NETWORK_PREFIX_ENV: &str = "RGB_NO_NETWORK_PREFIX";
pub const RGB_WALLET_ENV: &str = "RGB_WALLET";
pub const RGB_COINSELECT_STRATEGY_ENV: &str = "RGB_COINSELECT_STRATEGY";

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

    /// Bitcoin network
    #[arg(short, long, global = true, default_value = "testnet4", env = RGB_NETWORK_ENV)]
    pub network: Network,

    /// Do not add network name as a prefix to the data directory
    #[arg(long, global = true, env = RGB_NO_NETWORK_PREFIX_ENV)]
    pub no_network_prefix: bool,

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

    /// Generate an invoice
    Invoice {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Just generate a single-use seal, and not an entire invoice
        #[clap(long)]
        seal_only: bool,

        /// Use witness output-based seal
        #[clap(short = 'o', long)]
        wout: bool,

        /// Nonce number to use
        #[clap(long, global = true)]
        nonce: Option<u64>,

        /// Contract to use
        contract: Option<ContractRef>,

        /// API name to interface the contract
        ///
        /// If skipped, a default contract API will be used.
        #[clap(short, long, global = true)]
        api: Option<TypeName>,

        /// Method name to call the contract with
        ///
        /// If skipped, a default API method will be used.
        #[clap(short, long, global = true)]
        method: Option<MethodName>,

        /// State name used for the invoice
        ///
        /// If skipped, a default API state for the default method will be used.
        #[clap(short, long, global = true)]
        state: Option<StateName>,

        /// Invoiced state value
        value: Option<u64>,
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

    /// Pay an invoice, creating ready-to-be signed PSBT and a consignment
    #[clap(alias = "p")]
    Pay {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Coinselect strategy to use
        #[clap(short, long, default_value = "aggregate", env = RGB_COINSELECT_STRATEGY_ENV)]
        strategy: CoinselectStrategy,

        /// Invoice to fulfill
        invoice: RgbInvoice<ContractId>,

        /// Fees for PSBT
        fee: Sats,

        /// File to save the produced PSBT
        ///
        /// If not provided, prints PSBT to standard output.
        #[clap(value_hint = ValueHint::FilePath)]
        psbt: Option<PathBuf>,

        /// Location to save the consignment file to
        #[clap(value_hint = ValueHint::FilePath)]
        consignment: PathBuf,
    },

    /// Create a payment script out from invoice
    Script {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Coinselect strategy to use
        #[clap(short, long, default_value = "aggregate", env = RGB_COINSELECT_STRATEGY_ENV)]
        strategy: CoinselectStrategy,

        /// Invoice to fulfill
        invoice: RgbInvoice<ContractId>,

        /// Location to save the payment script to
        #[clap(value_hint = ValueHint::FilePath)]
        output: PathBuf,
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
    pub fn data_dir(&self) -> PathBuf {
        if self.no_network_prefix {
            self.data_dir.clone()
        } else {
            let mut dir = self.data_dir.join("bitcoin");
            if self.network.is_testnet() {
                dir.set_extension("testnet");
            }
            dir
        }
    }

    pub fn mound(&self) -> BpDirMound {
        if self.init {
            let _ = fs::create_dir_all(self.data_dir());
        }
        if !self.network.is_testnet() {
            panic!("Non-testnet networks are not yet supported");
        }
        BpDirMound::load_testnet(Consensus::Bitcoin, &self.data_dir, self.no_network_prefix)
    }

    fn wallet_dir(&self, name: Option<&str>) -> PathBuf {
        self.data_dir()
            .join(name.unwrap_or("default"))
            .with_extension("wallet")
    }

    pub fn wallet_provider(&self, name: Option<&str>) -> FsTextStore {
        FsTextStore::new(self.wallet_dir(name)).expect("Broken directory structure")
    }

    pub fn runtime(&self, name: Option<&str>) -> RgbDirRuntime {
        let provider = self.wallet_provider(name);
        let wallet = RgbWallet::load(provider, true).unwrap_or_else(|_| {
            panic!("Error: unable to load wallet from path `{}`", self.wallet_dir(name).display())
        });
        RgbDirRuntime::from(DirBarrow::with(wallet, self.mound()))
        // TODO: Sync wallet if needed
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
                let contract_id = runtime.issue_to_file(params)?;
                println!("A new contract issued with ID {contract_id}");
            }

            //Cmd::Import { articles } => self.mound().import_file(articles),
            //Cmd::Export { contract, file } => self.mound().export_file(contract, file),
            Cmd::Create { name, descriptor } => {
                let provider = self.wallet_provider(Some(name));
                let xpub = XpubDerivable::from_str(descriptor).expect("Invalid extended pubkey");
                let noise = xpub.xpub().chain_code().to_byte_array();
                RgbWallet::create(
                    provider,
                    RgbDescr::new_unfunded(Wpkh::from(xpub), noise),
                    self.network,
                    true,
                )
                .expect("Unable to create wallet");
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
                let addr = runtime.wallet.next_address(Keychain::OUTER, true);
                println!("{addr}");
            }

            Cmd::Invoice {
                wallet,
                seal_only,
                wout,
                nonce,
                contract,
                api,
                method,
                state,
                value,
            } => {
                let mut runtime = self.runtime(wallet.as_deref());
                let beneficiary = if *wout {
                    let wout = runtime.wout(*nonce);
                    RgbBeneficiary::WitnessOut(wout)
                } else {
                    let auth = runtime.auth_token(*nonce).ok_or(anyhow::anyhow!(
                        "Wallet has no unspent outputs; try `fund` first, or use `-w` flag to \
                         generate a witness output-based seal"
                    ))?;
                    RgbBeneficiary::Token(auth)
                };
                if *seal_only {
                    println!("{beneficiary}");
                    return Ok(());
                }

                let contract_id = if let Some(contract) = contract {
                    let id = runtime
                        .mound
                        .find_contract_id(contract.clone())
                        .ok_or(anyhow::anyhow!("unknown contract '{contract}'"))?;
                    CallScope::ContractId(id)
                } else {
                    CallScope::ContractQuery(s!(""))
                };
                let value = value.map(StrictVal::num);
                let mut invoice = RgbInvoice::new(contract_id, beneficiary, value);
                if let Some(api) = api {
                    invoice = invoice.use_api(api.clone());
                }
                if let Some(method) = method {
                    invoice = invoice.use_method(method.clone());
                }
                if let Some(state) = state {
                    invoice = invoice.use_state(state.clone());
                }

                println!("{invoice}");
            }

            Cmd::State { wallet, all, global, owned, contract } => {
                let mut runtime = self.runtime(wallet.wallet.as_deref());
                if wallet.sync {
                    let indexer = self.indexer(&wallet.resolver);
                    runtime.wallet.update(&indexer, false);
                    println!();
                }
                let state = if *all {
                    runtime.state_all(*contract).collect::<Vec<_>>()
                } else {
                    runtime.state_own(*contract).collect()
                };
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

            Cmd::Pay {
                wallet,
                strategy,
                invoice,
                fee,
                psbt: psbt_filename,
                consignment,
            } => {
                let mut runtime = self.runtime(wallet.wallet.as_deref());
                // TODO: sync wallet if needed
                // TODO: Add params and giveway to arguments
                let params = TxParams::with(*fee);
                let giveaway = Some(Sats::from(500u16));
                let (psbt, terminal) = runtime.pay_invoice(invoice, *strategy, params, giveaway)?;
                if let Some(psbt_filename) = psbt_filename {
                    psbt.encode(
                        psbt.version,
                        &mut File::create(psbt_filename).expect("Unable to write PSBT"),
                    )?;
                } else {
                    println!("{psbt}");
                }
                runtime
                    .mound
                    .consign_to_file(invoice.scope, [terminal], consignment)
                    .expect("Unable to consign contract");
            }

            Cmd::Script { wallet, strategy, invoice, output } => {
                let mut runtime = self.runtime(wallet.wallet.as_deref());
                let giveaway = Some(Sats::from(500u16));
                let script = runtime.script(invoice, *strategy, giveaway)?;
                let file = File::create_new(output).expect("Unable to open script file");
                serde_yaml::to_writer(file, &script).expect("Unable to write script");
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
                let script =
                    serde_yaml::from_reader::<_, OpRequestSet<Option<WoutAssignment>>>(src)?;

                let params = TxParams::with(*fee);
                let (psbt, bundle) = runtime.exec(script, params)?;
                let mut psbt_file = File::create_new(
                    psbt_filename
                        .as_ref()
                        .unwrap_or(bundle_filename)
                        .with_extension("psbt"),
                )
                .expect("Unable to create PSBT");

                bundle
                    .strict_serialize_to_file::<{ usize::MAX }>(&bundle_filename)
                    .expect("Unable to write output file");

                // This PSBT can be sent to other payjoin parties so they add their inputs and
                // outputs, or even re-order existing ones
                psbt.encode(psbt.version, &mut psbt_file)
                    .expect("Unable to write PSBT");
                if *print {
                    println!("{psbt}");
                }
            }

            Cmd::Complete { wallet, bundle, psbt: psbt_file } => {
                let mut runtime = self.runtime(wallet.as_deref());
                let bundle = PrefabBundle::strict_deserialize_from_file::<{ usize::MAX }>(bundle)?;
                let psbt = Psbt::decode(&mut File::open(psbt_file).expect("Unable to open PSBT"))?;

                let psbt = runtime.complete(psbt, &bundle)?;

                psbt.encode(
                    psbt.version,
                    &mut File::create(psbt_file).expect("Unable to write PSBT"),
                )?;
            }

            Cmd::Consign { contract, terminals, output } => {
                let mut mound = self.mound();
                mound
                    .consign_to_file(*contract, terminals, output)
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
