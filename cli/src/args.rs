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

#![allow(clippy::needless_update)] // Required by From derive macro

use std::fs;
use std::io::ErrorKind;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use bpstd::{Wpkh, XpubDerivable};
use bpwallet::cli::{Args as BpArgs, Config, DescriptorOpts};
use bpwallet::Wallet;
use nonasync::persistence::PersistenceError;
use rgb::persistence::Stock;
use rgb::resolvers::AnyResolver;
use rgb::{RgbDescr, RgbWallet, TapretKey, WalletError};
use rgbstd::persistence::fs::FsBinStore;
use strict_types::encoding::{DecodeError, DeserializeError};

use crate::Command;

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group()]
pub struct DescrRgbOpts {
    /// Use tapret(KEY) descriptor as wallet.
    #[arg(long, global = true)]
    pub tapret_key_only: Option<XpubDerivable>,

    /// Use wpkh(KEY) descriptor as wallet.
    #[arg(long, global = true)]
    pub wpkh: Option<XpubDerivable>,
}

impl DescriptorOpts for DescrRgbOpts {
    type Descr = RgbDescr;

    fn is_some(&self) -> bool { self.tapret_key_only.is_some() || self.wpkh.is_some() }

    fn descriptor(&self) -> Option<Self::Descr> {
        self.tapret_key_only
            .clone()
            .map(TapretKey::from)
            .map(TapretKey::into)
            .or(self.wpkh.clone().map(Wpkh::from).map(Wpkh::into))
    }
}

/// Command-line arguments
#[derive(Parser)]
#[derive(Clone, Eq, PartialEq, Debug)]
#[command(author, version, about)]
pub struct RgbArgs {
    #[clap(flatten)]
    pub inner: BpArgs<Command, DescrRgbOpts>,

    /// Specify blockchain height starting from which witness transactions
    /// should be checked for re-orgs
    #[clap(short = 'H', long, requires = "sync")]
    pub from_height: Option<u32>,
}

impl Deref for RgbArgs {
    type Target = BpArgs<Command, DescrRgbOpts>;
    #[inline]
    fn deref(&self) -> &Self::Target { &self.inner }
}

impl DerefMut for RgbArgs {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.inner }
}

impl Default for RgbArgs {
    fn default() -> Self { unreachable!() }
}

impl RgbArgs {
    pub(crate) fn load_stock(
        &self,
        stock_path: impl ToOwned<Owned = PathBuf>,
    ) -> Result<Stock, WalletError> {
        let stock_path = stock_path.to_owned();

        if self.verbose > 1 {
            eprint!("Loading stock from `{}` ... ", stock_path.display());
        }

        let provider = FsBinStore::new(stock_path.clone())?;
        let mut stock = Stock::load(provider, true)
            .map_err(WalletError::WalletPersist)
            .or_else(|err| {
                if let WalletError::WalletPersist(PersistenceError(ref err)) = err {
                    if let Some(DeserializeError::Decode(DecodeError::Io(io_err))) =
                        err.downcast_ref::<DeserializeError>()
                    {
                        if io_err.kind() == ErrorKind::NotFound {
                            if self.verbose > 1 {
                                eprint!("stock file is absent, creating a new one ... ");
                            }
                            fs::create_dir_all(&stock_path)?;
                            let provider = FsBinStore::new(stock_path)?;
                            let mut stock = Stock::in_memory();
                            stock
                                .make_persistent(provider, true)
                                .map_err(WalletError::StockPersist)?;
                            return Ok(stock);
                        }
                    }
                }
                eprintln!("stock file is damaged, failing");
                Err(err)
            })?;

        if self.sync {
            let resolver = self.resolver()?;
            let from_height = self.from_height.unwrap_or(1);
            eprint!("Updating witness information starting from height {from_height} ... ");
            let res = stock.update_witnesses(resolver, from_height)?;
            eprint!("{} transactions were checked and updated", res.succeeded);
            if res.failed.is_empty() {
                eprintln!();
            } else {
                eprintln!(", {} resolution failures:", res.failed.len());
                for (witness_id, failure) in res.failed {
                    eprintln!(" - {witness_id}: {failure}");
                }
            }
        }

        if self.verbose > 1 {
            eprintln!("success");
        }

        Ok(stock)
    }

    pub fn rgb_stock(&self) -> Result<Stock, WalletError> {
        let stock_path = self.general.base_dir();
        let stock = self.load_stock(stock_path)?;
        Ok(stock)
    }

    pub fn rgb_wallet(
        &self,
        config: &Config,
    ) -> Result<RgbWallet<Wallet<XpubDerivable, RgbDescr>>, WalletError> {
        let stock = self.rgb_stock()?;
        self.rgb_wallet_from_stock(config, stock)
            .map_err(|(_, err)| err)
    }

    pub fn rgb_wallet_from_stock(
        &self,
        config: &Config,
        stock: Stock,
    ) -> Result<RgbWallet<Wallet<XpubDerivable, RgbDescr>>, (Stock, WalletError)> {
        let wallet = match self.inner.bp_wallet::<RgbDescr>(config) {
            Ok(wallet) => wallet,
            Err(e) => return Err((stock, e.into())),
        };
        let wallet = RgbWallet::new(stock, wallet);

        Ok(wallet)
    }

    pub fn resolver(&self) -> Result<AnyResolver, WalletError> {
        let resolver =
            match (&self.resolver.esplora, &self.resolver.electrum, &self.resolver.mempool) {
                (None, Some(url), None) => AnyResolver::electrum_blocking(url, None),
                (Some(url), None, None) => AnyResolver::esplora_blocking(url, None),
                (None, None, Some(url)) => AnyResolver::mempool_blocking(url, None),
                _ => Err(s!(" - error: no transaction resolver is specified; use either \
                             --esplora --mempool or --electrum argument")),
            }
            .map_err(WalletError::Resolver)?;
        resolver.check(self.general.network)?;
        Ok(resolver)
    }
}
