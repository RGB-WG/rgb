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

use bpwallet::Sats;
use clap::ValueHint;
use rgb::invoice::RgbInvoice;
use rgb::{AuthToken, ContractId, ContractRef, MethodName, StateName};
use rgbp::CoinselectStrategy;
use strict_encoding::TypeName;

use crate::opts::WalletOpts;

pub const RGB_COINSELECT_STRATEGY_ENV: &str = "RGB_COINSELECT_STRATEGY";
pub const RGB_WALLET_ENV: &str = "RGB_WALLET";

#[derive(Parser)]
pub enum Cmd {
    // =====================================================================================
    // I. Wallet management
    /// List known wallets
    Wallets,

    /// Create a new wallet
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

    /// Receiving a wallet address for gas funding
    Fund {
        /// Wallet to use
        #[clap(env = RGB_WALLET_ENV)]
        wallet: Option<String>,
    },

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
        contract: ContractRef,

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
        contract: ContractRef,
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

    /// Pay an invoice, creating ready-to-be signed PSBT and a consignment
    #[clap(alias = "p")]
    Pay {
        #[clap(flatten)]
        wallet: WalletOpts,

        /// Coinselect strategy to use
        #[clap(short, long, default_value = "aggregate", env = RGB_COINSELECT_STRATEGY_ENV)]
        strategy: CoinselectStrategy,

        /// Amount of sats to send to pay-to-address invoice
        #[clap(long, global = true)]
        sats: Option<Sats>,

        /// Fees for PSBT
        #[clap(long, global = true, default_value = "1000")]
        fee: Sats,

        /// Invoice to fulfill
        invoice: RgbInvoice<ContractId>,

        /// Location to save the consignment file to
        #[clap(value_hint = ValueHint::FilePath)]
        consignment: PathBuf,

        /// File to save the produced PSBT
        ///
        /// If not provided, prints PSBT to standard output.
        #[clap(value_hint = ValueHint::FilePath)]
        psbt: Option<PathBuf>,
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
        contract: ContractRef,

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
