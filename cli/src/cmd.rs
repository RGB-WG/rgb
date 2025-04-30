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

use std::path::PathBuf;

use bpwallet::cli::ResolverOpt;
use bpwallet::Sats;
use clap::ValueHint;
use rgb::invoice::RgbInvoice;
use rgb::{AuthToken, ContractId, ContractRef, MethodName, StateName};
use rgbp::CoinselectStrategy;
use strict_encoding::TypeName;

use crate::opts::WalletOpts;

pub const RGB_COINSELECT_STRATEGY_ENV: &str = "RGB_COINSELECT_STRATEGY";
pub const RGB_WALLET_ENV: &str = "RGB_WALLET";
pub const RGB_PSBT_VER: &str = "RGB_PSBT_VER2";

#[derive(PartialEq, Eq, Parser)]
pub enum Cmd {
    /// Initialize data directory
    ///
    /// The command will fail if the directory already exists.
    Init {
        /// Do not print error messages if the directory already exists; and just do nothing.
        ///
        /// NB: This will still produce an error if the directory is absent and can't be created
        /// (for instance, due to a lack of access rights to one of the parent directories).
        #[clap(short, long)]
        quiet: bool,
    },

    // =====================================================================================
    // I. Wallet management
    /// List known wallets
    Wallets,

    /// Create a new wallet from a descriptor
    Create {
        #[clap(long, conflicts_with = "wpkh")]
        tapret_key_only: bool,

        #[clap(long)]
        wpkh: bool,

        /// Wallet name
        name: String,

        /// Extended pubkey descriptor
        descriptor: String,
    },

    /// Synchronize wallet and contracts with the blockchain
    Sync {
        #[clap(flatten)]
        resolver: ResolverOpt,

        /// Wallet to use
        #[clap(env = RGB_WALLET_ENV)]
        wallet: Option<String>,
    },

    /// Receiving a wallet address for gas funding
    Fund {
        /// Wallet to use
        #[clap(env = RGB_WALLET_ENV)]
        wallet: Option<String>,
    },

    /// List available wallet seals
    Seals {
        #[clap(flatten)]
        wallet: WalletOpts,
    },

    // =====================================================================================
    // II. Contract management
    /// List contracts
    Contracts {
        /// Include in the list contract issuing schemata
        #[clap(short, long)]
        issuers: bool,
    },

    /// Issue a new RGB contract
    Issue {
        /// Do not print error messages if something goes wrong
        #[clap(short, long)]
        quiet: bool,

        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Parameters and data for the contract
        #[clap(value_hint = ValueHint::FilePath)]
        params: Option<PathBuf>,
    },

    /// Remove a contract purging all its data (use with caution!)
    Purge {
        /// Force removal of a contract with a known state
        #[clap(short, long)]
        force: bool,

        /// Contract id to remove
        contract: ContractId,
    },

    /// Import contract issuer schema(ta)
    ///
    /// If you need to import a contract, please use the `accept` command.
    Import {
        /// File(s) to process
        #[clap(value_hint = ValueHint::FilePath)]
        file: Vec<PathBuf>,
    },

    /// Export a contract as a consignment
    Export {
        /// Contract to export
        contract: ContractRef,

        /// Path to save the contract consignment to
        #[clap(value_hint = ValueHint::FilePath)]
        file: Option<PathBuf>,
    },

    /// Back up all client-side data for all contracts
    Backup {
        /// Path for saving a backup in the form of a tar file
        #[clap(default_value = "rgb-backup.tar", value_hint = ValueHint::FilePath)]
        file: PathBuf,
    },

    // =====================================================================================
    // III. Combined contract/wallet operations
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
        contract: Option<ContractRef>,
    },

    /// Generate an invoice
    #[clap(alias = "i")]
    Invoice {
        /// Wallet to use
        #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
        wallet: Option<String>,

        /// Just generate a single-use seal, and not an entire invoice
        #[clap(long)]
        seal_only: bool,

        /// Use witness-output-based seal
        #[clap(long)]
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

    /// Pay an invoice, creating ready-to-be signed PSBT and a consignment
    #[clap(alias = "p")]
    Pay {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Coinselect strategy to use
        #[clap(short, long, default_value = "aggregate", env = RGB_COINSELECT_STRATEGY_ENV)]
        strategy: CoinselectStrategy,

        /// Amount of sats to send to pay-to-address invoice
        #[clap(long)]
        sats: Option<Sats>,

        /// Fees for PSBT
        #[clap(long, default_value = "1000")]
        fee: Sats,

        /// Use PSBT version 2
        #[clap(short = '2', long, env = "RGB_PSBT_VER2")]
        psbt2: bool,

        /// Print PSBT to STDOUT
        #[clap(short, long)]
        print: bool,

        /// Force re-rewrite of PSBT or a consignment if any of the files already exist
        #[clap(short, long)]
        force: bool,

        /// Invoice to fulfill
        invoice: RgbInvoice<ContractId>,

        /// Location to save the consignment file to
        #[clap(value_hint = ValueHint::FilePath)]
        consignment: PathBuf,

        /// File to save the produced PSBT
        ///
        /// If not provided, uses the same path and filename as for the consignment, adding *.psbt
        /// extension.
        #[clap(value_hint = ValueHint::FilePath)]
        psbt: Option<PathBuf>,
    },

    /// Create a payment script out from invoice
    Script {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Amount of sats to send to pay-to-address invoice
        #[clap(long, global = true)]
        sats: Option<Sats>,

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

        /// Use PSBT version 2
        #[clap(short = '2', long, global = true, env = "RGB_PSBT_VER2")]
        psbt2: bool,

        /// Print PSBT to STDOUT
        #[clap(short, long, global = true)]
        print: bool,

        /// File to save the produced PSBT
        ///
        /// If not provided, uses the same filename as for the bundle, replacing the extension with
        /// 'psbt'.
        #[clap(value_hint = ValueHint::FilePath)]
        psbt: Option<PathBuf>,
    },

    /// Complete finalizes PSBT and adds information about witness to the contracts' store
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
        contract: ContractRef,

        /// List of tokens of authority which should serve as a contract terminals
        #[clap(short, long)]
        terminals: Vec<AuthToken>,

        /// Location to save the consignment file to
        #[clap(value_hint = ValueHint::FilePath)]
        output: PathBuf,
    },

    /// Finalize signed PSBT, extract raw transaction and, optionally, broadcast it
    Finalize {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Broadcast the transaction.
        #[clap(short, long, global = true)]
        broadcast: bool,

        /// Name of PSBT file to finalize.
        psbt: PathBuf,

        /// File to save the extracted signed transaction.
        tx: Option<PathBuf>,
    },

    /// Verify and accept a contract or a transfer consignment
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
