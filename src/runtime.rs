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

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs::{self, File};
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use bitcoin::bip32::ExtendedPubKey;
use rgbfs::StockFs;
use rgbstd::containers::{Contract, LoadError, Transfer};
use rgbstd::interface::BuilderError;
use rgbstd::persistence::{Inventory, InventoryDataError, InventoryError, StashError, Stock};
use rgbstd::{validation, Chain};
use strict_types::encoding::{DeserializeError, Ident, SerializeError};

use crate::descriptor::RgbDescr;
use crate::{BlockchainResolver, RgbWallet, Tapret};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum RuntimeError {
    #[from]
    Io(io::Error),

    #[from]
    Yaml(serde_yaml::Error),

    #[from]
    Serialize(SerializeError),

    #[from]
    Deserialize(DeserializeError),

    #[from]
    Load(LoadError),

    #[from]
    Stash(StashError<Infallible>),

    #[from]
    #[from(InventoryDataError<Infallible>)]
    Inventory(InventoryError<Infallible>),

    #[from]
    Builder(BuilderError),

    #[display(doc_comments)]
    /// wallet with id '{0}' is not known to the system
    WalletUnknown(Ident),

    #[from]
    Psbt(bitcoin::psbt::Error),

    #[cfg(feature = "electrum")]
    #[from]
    Electrum(electrum_client::Error),

    #[from]
    InvalidConsignment(validation::Status),

    #[from]
    Custom(String),
}

impl From<Infallible> for RuntimeError {
    fn from(_: Infallible) -> Self { unreachable!() }
}

#[derive(Getters)]
pub struct Runtime {
    stock_path: PathBuf,
    wallets_path: PathBuf,
    #[getter(skip)]
    stock: Stock,
    wallets: HashMap<Ident, RgbDescr>,
    #[getter(as_copy)]
    chain: Chain,
    #[getter(skip)]
    resolver: BlockchainResolver,
}

impl Deref for Runtime {
    type Target = Stock;
    fn deref(&self) -> &Self::Target { &self.stock }
}

impl DerefMut for Runtime {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.stock }
}

impl Runtime {
    pub fn load(mut data_dir: PathBuf, chain: Chain, electrum: &str) -> Result<Self, RuntimeError> {
        data_dir.push(chain.to_string());
        debug!("Using data directory '{}'", data_dir.display());
        fs::create_dir_all(&data_dir)?;

        let mut stock_path = data_dir.clone();
        stock_path.push("stock.dat");
        debug!("Reading stock from '{}'", stock_path.display());
        let stock = if !stock_path.exists() {
            eprintln!("Stock file not found, creating default stock");
            let stock = Stock::default();
            stock.store(&stock_path)?;
            stock
        } else {
            Stock::load(&stock_path)?
        };

        let mut wallets_path = data_dir.clone();
        wallets_path.push("wallets.yml");
        debug!("Reading wallets from '{}'", wallets_path.display());
        let wallets = if !wallets_path.exists() {
            eprintln!("Wallet file not found, creating new wallet list");
            empty!()
        } else {
            let wallets_fd = File::open(&wallets_path)?;
            serde_yaml::from_reader(&wallets_fd)?
        };

        let resolver = BlockchainResolver::with(electrum)?;

        Ok(Self {
            stock_path,
            wallets_path,
            stock,
            wallets,
            chain,
            resolver,
        })
    }

    pub fn unload(self) -> () {}

    pub fn resolver(&mut self) -> &mut BlockchainResolver { &mut self.resolver }

    pub fn create_wallet(
        &mut self,
        name: &Ident,
        xpub: ExtendedPubKey,
    ) -> Result<&RgbDescr, RuntimeError> {
        let descr = RgbDescr::Tapret(Tapret {
            xpub,
            taprets: empty!(),
        });
        let entry = match self.wallets.entry(name.clone()) {
            Entry::Occupied(_) => return Err(format!("wallet named {name} already exists").into()),
            Entry::Vacant(entry) => entry.insert(descr),
        };
        Ok(entry)
    }

    pub fn wallet(&mut self, name: &Ident) -> Result<RgbWallet, RuntimeError> {
        let descr = self
            .wallets
            .get(name)
            .ok_or(RuntimeError::WalletUnknown(name.clone()))?;
        RgbWallet::with(descr.clone(), &mut self.resolver).map_err(RuntimeError::from)
    }

    pub fn import_contract(
        &mut self,
        contract: Contract,
    ) -> Result<validation::Status, RuntimeError> {
        self.stock
            .import_contract(contract, &mut self.resolver)
            .map_err(RuntimeError::from)
    }

    pub fn validate_transfer<'transfer>(
        &mut self,
        transfer: Transfer,
    ) -> Result<Transfer, RuntimeError> {
        transfer
            .validate(&mut self.resolver)
            .map_err(|invalid| invalid.validation_status().expect("just validated").clone())
            .map_err(RuntimeError::from)
    }

    pub fn accept_transfer(
        &mut self,
        transfer: Transfer,
        force: bool,
    ) -> Result<validation::Status, RuntimeError> {
        self.stock
            .accept_transfer(transfer, &mut self.resolver, force)
            .map_err(RuntimeError::from)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stock
            .store(&self.stock_path)
            .expect("unable to save stock");
        let wallets_fd = File::create(&self.wallets_path)
            .expect("unable to access wallet file; wallets are not saved");
        serde_yaml::to_writer(wallets_fd, &self.wallets).expect("unable to save wallets");
    }
}
