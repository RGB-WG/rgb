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
use std::path::PathBuf;
use std::process::exit;

use bpwallet::cli::ResolverOpt;
use bpwallet::fs::FsTextStore;
use bpwallet::seals::TxoSeal;
use bpwallet::{AnyIndexer, Network};
use clap::ValueHint;
use rgb::persistance::StockFs;
use rgb::popls::bp::RgbWallet;
use rgb::{Consensus, ContractsInmem, PileFs};
use rgbp::{Owner, RgbDirRuntime};

use crate::cmd::Cmd;
use crate::opts::WalletOpts;

pub const RGB_NETWORK_ENV: &str = "RGB_NETWORK";
pub const RGB_NO_NETWORK_PREFIX_ENV: &str = "RGB_NO_NETWORK_PREFIX";

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

    pub fn contracts(&self) -> ContractsInmem<StockFs, PileFs<TxoSeal>> {
        if self.init {
            let _ = fs::create_dir_all(self.data_dir());
        }
        if !self.network.is_testnet() {
            panic!("Non-testnet networks are not yet supported");
        }
        ContractsInmem::with_testnet_dir(
            Consensus::Bitcoin,
            self.data_dir.clone(),
            self.no_network_prefix,
        )
    }

    fn wallet_dir(&self, name: Option<&str>) -> PathBuf {
        self.data_dir()
            .join(name.unwrap_or("default"))
            .with_extension("wallet")
    }

    pub fn wallet_provider(&self, name: Option<&str>) -> FsTextStore {
        FsTextStore::new(self.wallet_dir(name)).expect("Broken directory structure")
    }

    pub fn runtime(&self, opts: &WalletOpts) -> RgbDirRuntime {
        let provider = self.wallet_provider(opts.wallet.as_deref());
        let wallet = Owner::load(provider, true).unwrap_or_else(|_| {
            panic!(
                "Error: unable to load wallet from path `{}`",
                self.wallet_dir(opts.wallet.as_deref()).display()
            )
        });
        let mut runtime = RgbDirRuntime::from(RgbWallet::with(wallet, self.contracts()));
        if opts.sync {
            eprint!("Synchronizing wallet:");
            let indexer = self.indexer(&opts.resolver);
            runtime
                .sync(&indexer)
                .expect("Unable to synchronize wallet");
            eprintln!(" done");
        }
        runtime
    }

    pub fn indexer(&self, resolver: &ResolverOpt) -> AnyIndexer {
        let network = self.network.to_string();
        match (&resolver.esplora, &resolver.electrum, &resolver.mempool) {
            (None, Some(url), None) => AnyIndexer::Electrum(Box::new(
                // TODO: Check network match
                electrum::Client::new(url).expect("Unable to initialize indexer"),
            )),
            (Some(url), None, None) => AnyIndexer::Esplora(Box::new(
                bpwallet::indexers::esplora::Client::new_esplora(
                    &url.replace("{network}", &network),
                )
                .expect("Unable to initialize indexer"),
            )),
            (None, None, Some(url)) => AnyIndexer::Mempool(Box::new(
                bpwallet::indexers::esplora::Client::new_mempool(
                    &url.replace("{network}", &network),
                )
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
}
