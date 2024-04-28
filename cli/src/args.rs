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
use std::path::Path;

use bpstd::{Wpkh, XpubDerivable};
use bpwallet::cli::{Args as BpArgs, Config, DescriptorOpts};
use bpwallet::Wallet;
use rgb::{
    electrum, esplora_blocking, AnyResolver, AnyResolverError, RgbDescr, StoredStock, StoredWallet,
    TapretKey, WalletError,
};
use rgbstd::persistence::fs::{LoadFs, StoreFs};
use rgbstd::persistence::Stock;
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
#[derive(Wrapper, WrapperMut, Clone, Eq, PartialEq, Debug, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
#[command(author, version, about)]
pub struct RgbArgs {
    #[clap(flatten)]
    pub inner: BpArgs<Command, DescrRgbOpts>,
}

impl Default for RgbArgs {
    fn default() -> Self { unreachable!() }
}

impl RgbArgs {
    fn load_stock(&self, stock_path: &Path) -> Result<Stock, WalletError> {
        if self.verbose > 1 {
            eprint!("Loading stock ... ");
        }

        Stock::load(&stock_path).map_err(WalletError::from).or_else(|err| {
            if matches!(err, WalletError::Deserialize(DeserializeError::Decode(DecodeError::Io(ref err))) if err.kind() == ErrorKind::NotFound) {
                if self.verbose > 1 {
                    eprint!("stock file is absent, creating a new one ... ");
                }
                let stock = Stock::default();
                fs::create_dir_all(&stock_path)?;
                stock.store(&stock_path)?;
                if self.verbose > 1 {
                    eprintln!("success");
                }
                return Ok(stock)
            }
            eprintln!("stock file is damaged, failing");
            Err(err)
        })
    }

    pub fn rgb_stock(&self) -> Result<StoredStock, WalletError> {
        let stock_path = self.general.base_dir();
        let stock = self.load_stock(&stock_path)?;
        Ok(StoredStock::attach(stock_path, stock))
    }

    pub fn rgb_wallet(
        &self,
        config: &Config,
    ) -> Result<StoredWallet<Wallet<XpubDerivable, RgbDescr>>, WalletError> {
        let stock_path = self.general.base_dir();
        let stock = self.load_stock(&stock_path)?;
        let wallet = self.inner.bp_runtime::<RgbDescr>(config)?.detach();
        let wallet = StoredWallet::attach(stock_path, stock, wallet);

        Ok(wallet)
    }

    #[allow(clippy::result_large_err)]
    pub fn resolver(&self) -> Result<AnyResolver, AnyResolverError> {
        if self.resolver.electrum != bpwallet::cli::DEFAULT_ELECTRUM {
            match electrum::Resolver::new(&self.resolver.electrum) {
                Ok(c) => Ok(AnyResolver::Electrum(Box::new(c))),
                Err(e) => Err(AnyResolverError::Electrum(e)),
            }
        } else {
            match esplora_blocking::Resolver::new(&self.resolver.esplora) {
                Ok(c) => Ok(AnyResolver::Esplora(Box::new(c))),
                Err(e) => Err(AnyResolverError::Esplora(e)),
            }
        }
    }
}
