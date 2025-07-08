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
use std::{fs, io};

use bpstd::seals::TxoSeal;
use bpstd::Network;
use clap::ValueHint;
use rgb::popls::bp::RgbWallet;
use rgb::{Consensus, Contracts};
use rgb_persist_fs::StockpileDir;
use rgbp::resolvers::MultiResolver;
use rgbp::{FileOwner, RgbpRuntimeDir};

use crate::cmd::Cmd;
use crate::opts::{ResolverOpt, WalletOpts};

pub const RGB_NETWORK_ENV: &str = "RGB_NETWORK";
pub const RGB_NO_NETWORK_PREFIX_ENV: &str = "RGB_NO_NETWORK_PREFIX";

pub const RGB_DATA_DIR_ENV: &str = "RGB_DATA_DIR";
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub const RGB_DATA_DIR: &str = "~/.local/share/rgb";
#[cfg(target_os = "macos")]
pub const RGB_DATA_DIR: &str = "~/Library/Application Support/RGB Smart Contracts";
#[cfg(target_os = "windows")]
pub const RGB_DATA_DIR: &str = "~\\AppData\\Local\\RGB Smart Contracts";
#[cfg(target_os = "ios")]
pub const RGB_DATA_DIR: &str = "~/Documents";
#[cfg(target_os = "android")]
pub const RGB_DATA_DIR: &str = ".";

// Uses XDG_DATA_HOME if set, otherwise falls back to RGB_DATA_DIR
fn default_data_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("rgb")
    } else {
        PathBuf::from(RGB_DATA_DIR)
    }
}

#[derive(Parser)]
pub struct Args {
    /// Location of the data directory
    #[clap(
        short,
        long,
        global = true,
        default_value_os_t = default_data_dir(),
        env = RGB_DATA_DIR_ENV,
        value_hint = ValueHint::DirPath
    )]
    pub data_dir: PathBuf,

    /// Bitcoin network
    #[arg(short, long, global = true, default_value = "testnet4", env = RGB_NETWORK_ENV)]
    pub network: Network,

    /// Do not add network name as a prefix to the data directory
    #[arg(long, global = true, env = RGB_NO_NETWORK_PREFIX_ENV)]
    pub no_network_prefix: bool,

    /// Minimal number of confirmations to consider an operation final
    #[arg(long, global = true, default_value = "32")]
    pub min_confirmations: u32,

    /// Command to execute
    #[clap(subcommand)]
    pub command: Cmd,
}

impl Args {
    pub fn check_data_dir(&self) -> anyhow::Result<()> {
        let data_dir = self.data_dir();
        if !data_dir.is_dir() {
            if let Cmd::Init { quiet: _ } = self.command {
                fs::create_dir_all(self.data_dir()).map_err(|e| {
                    io::Error::new(
                        e.kind(),
                        format!("unable to initialize data directory at '{}'", data_dir.display()),
                    )
                })?;
            } else {
                anyhow::bail!(
                    "the data directory at '{}' is not initialized; please initialize it with \
                     `init` command or change the path using `--data-dir` argument",
                    data_dir.display()
                );
            }
        } else if let Cmd::Init { quiet: true } = self.command {
            anyhow::bail!("data directory at '{}' is already initialized", data_dir.display());
        }
        Ok(())
    }

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

    pub fn contracts(&self) -> Contracts<StockpileDir<TxoSeal>> {
        if !self.network.is_testnet() {
            panic!("Non-testnet networks are not yet supported");
        }
        let stockpile = StockpileDir::load(self.data_dir(), Consensus::Bitcoin, true)
            .expect("Invalid contracts directory");
        Contracts::load(stockpile)
    }

    fn wallet_dir(&self, name: Option<&str>) -> PathBuf {
        self.data_dir()
            .join(name.unwrap_or("default"))
            .with_extension("wallet")
    }

    pub fn runtime(&self, opts: &WalletOpts) -> RgbpRuntimeDir<MultiResolver> {
        let resolver = self.resolver(&opts.resolver);
        let path = self.wallet_dir(opts.wallet.as_deref());
        let wallet = FileOwner::load(path, self.network, resolver).unwrap_or_else(|err| {
            panic!(
                "unable to load wallet from path `{}`\nDetails: {err}",
                self.wallet_dir(opts.wallet.as_deref()).display()
            )
        });
        let mut runtime =
            RgbpRuntimeDir::from(RgbWallet::with_components(wallet, self.contracts()));
        if opts.sync {
            eprint!("Synchronizing wallet:");
            runtime
                .update(self.min_confirmations)
                .expect("Unable to synchronize wallet");
            eprintln!(" done");
        }
        runtime
    }

    pub fn resolver(&self, resolver: &ResolverOpt) -> MultiResolver {
        let network = self.network.to_string();
        match (&resolver.esplora, &resolver.electrum, &resolver.mempool) {
            (None, Some(url), None) => MultiResolver::new_electrum(url),
            (Some(url), None, None) => {
                MultiResolver::new_esplora(&url.replace("{network}", &network))
            }
            (None, None, Some(url)) => {
                MultiResolver::new_mempool(&url.replace("{network}", &network))
            }
            _ => MultiResolver::new_absent(),
        }
    }
}
