// RGB smart contract wallet runtime
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

use std::convert::Infallible;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::{fs, io};

use bp::{DeriveSpk, Outpoint};
use rgb::containers::LoadError;
use rgb::interface::{BuilderError, OutpointFilter};
use rgb::persistence::{InventoryDataError, InventoryError, StashError, Stock};
use rgb::{validation, Chain};
use rgbfs::StockFs;
use strict_types::encoding::{DeserializeError, Ident, SerializeError};

use crate::DescriptorRgb;

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum RuntimeError {
    #[from]
    Io(io::Error),

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

    /// wallet with id '{0}' is not known to the system
    #[display(doc_comments)]
    WalletUnknown(Ident),

    #[from]
    InvalidConsignment(validation::Status),

    /// the contract source doesn't provide all state information required by
    /// the schema. This means that some of the global fields or assignments are
    /// missed.
    #[display(doc_comments)]
    IncompleteContract,

    #[from]
    Bp(bp_rt::LoadError),

    #[from]
    Yaml(serde_yaml::Error),

    #[from]
    Custom(String),
}

impl From<Infallible> for RuntimeError {
    fn from(_: Infallible) -> Self { unreachable!() }
}

#[derive(Getters)]
pub struct Runtime<D: DeriveSpk = DescriptorRgb> {
    stock_path: PathBuf,
    stock: Stock,
    #[getter(as_mut)]
    wallet: Option<bp_rt::Runtime<D>>,
    #[getter(as_copy)]
    chain: Chain,
}

impl<D: DeriveSpk> Deref for Runtime<D> {
    type Target = Stock;

    fn deref(&self) -> &Self::Target { &self.stock }
}

impl<D: DeriveSpk> DerefMut for Runtime<D> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.stock }
}

impl<D: DeriveSpk> OutpointFilter for Runtime<D> {
    fn include_outpoint(&self, outpoint: Outpoint) -> bool {
        self.wallet
            .as_ref()
            .map(|rt| rt.wallet().coins().any(|utxo| utxo.outpoint == outpoint))
            .unwrap_or_default()
    }
}

impl<D: DeriveSpk> Runtime<D> {
    pub fn load(mut data_dir: PathBuf, chain: Chain) -> Result<Self, RuntimeError> {
        data_dir.push(chain.to_string());
        #[cfg(feature = "log")]
        debug!("Using data directory '{}'", data_dir.display());
        fs::create_dir_all(&data_dir)?;

        let mut stock_path = data_dir.clone();
        stock_path.push("stock.dat");
        #[cfg(feature = "log")]
        debug!("Reading stock from '{}'", stock_path.display());
        let stock = if !stock_path.exists() {
            eprintln!("Stock file not found, creating default stock");
            let stock = Stock::default();
            stock.store(&stock_path)?;
            stock
        } else {
            Stock::load(&stock_path)?
        };

        Ok(Self {
            stock_path,
            stock,
            wallet: None,
            chain,
        })
    }

    pub fn attach(&mut self, wallet: bp_rt::Runtime<D>) { self.wallet = Some(wallet) }

    pub fn detach(&mut self) { self.wallet = None; }

    pub fn unload(self) -> () {}

    /*
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
        Ok(RgbWallet::new(descr.clone()))
    }

    pub fn import_contract(
        &mut self,
        contract: Contract,
        resolver: &mut BlockchainResolver,
    ) -> Result<validation::Status, RuntimeError> {
        self.stock
            .import_contract(contract, resolver)
            .map_err(RuntimeError::from)
    }

    pub fn validate_transfer<'transfer>(
        &mut self,
        transfer: Transfer,
        resolver: &mut BlockchainResolver,
    ) -> Result<Transfer, RuntimeError> {
        transfer
            .validate(resolver)
            .map_err(|invalid| invalid.validation_status().expect("just validated").clone())
            .map_err(RuntimeError::from)
    }

    pub fn accept_transfer(
        &mut self,
        transfer: Transfer,
        resolver: &mut BlockchainResolver,
        force: bool,
    ) -> Result<validation::Status, RuntimeError> {
        self.stock
            .accept_transfer(transfer, resolver, force)
            .map_err(RuntimeError::from)
    }
     */
}

impl<D: DeriveSpk> Drop for Runtime<D> {
    fn drop(&mut self) {
        self.stock
            .store(&self.stock_path)
            .expect("unable to save stock");
        /*
        let wallets_fd = File::create(&self.wallets_path)
            .expect("unable to access wallet file; wallets are not saved");
        serde_yaml::to_writer(wallets_fd, &self.wallets).expect("unable to save wallets");
         */
    }
}
