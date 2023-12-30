// RGB wallet library for smart contracts on Bitcoin & Lightning network
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

#![allow(clippy::result_large_err)]

use std::convert::Infallible;
use std::io;
use std::io::ErrorKind;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use amplify::IoError;
use bpstd::{Network, XpubDerivable};
use bpwallet::Wallet;
use rgbfs::StockFs;
use rgbstd::containers::{Contract, LoadError, Transfer};
use rgbstd::interface::{BuilderError, OutpointFilter};
use rgbstd::persistence::{Inventory, InventoryDataError, InventoryError, StashError, Stock};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{self, ResolveWitness};
use rgbstd::{ContractId, XChain, XOutpoint};
use strict_types::encoding::{DecodeError, DeserializeError, Ident, SerializeError};

use crate::{DescriptorRgb, RgbDescr};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum RuntimeError {
    #[from]
    #[from(io::Error)]
    Io(IoError),

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

    #[from]
    PsbtDecode(psbt::DecodeError),

    /// wallet with id '{0}' is not known to the system.
    #[display(doc_comments)]
    WalletUnknown(Ident),

    #[from]
    InvalidConsignment(validation::Status),

    /// invalid identifier.
    #[from]
    #[display(doc_comments)]
    InvalidId(baid58::Baid58ParseError),

    /// the contract source doesn't fit requirements imposed by the used schema.
    ///
    /// {0}
    #[display(doc_comments)]
    IncompleteContract(validation::Status),

    #[from]
    #[from(bpwallet::LoadError)]
    Bp(bpwallet::RuntimeError),

    #[cfg(feature = "esplora")]
    #[from]
    Esplora(esplora::Error),

    #[from]
    Yaml(serde_yaml::Error),

    #[from]
    Custom(String),
}

impl From<Infallible> for RuntimeError {
    fn from(_: Infallible) -> Self { unreachable!() }
}

#[derive(Getters)]
pub struct Runtime<D: DescriptorRgb<K> = RgbDescr, K = XpubDerivable> {
    stock_path: PathBuf,
    #[getter(as_mut)]
    stock: Stock,
    bprt: bpwallet::Runtime<D, K /* TODO: Add layer 2 */>,
}

impl<D: DescriptorRgb<K>, K> Deref for Runtime<D, K> {
    type Target = Stock;

    fn deref(&self) -> &Self::Target { &self.stock }
}

impl<D: DescriptorRgb<K>, K> DerefMut for Runtime<D, K> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.stock }
}

impl<D: DescriptorRgb<K>, K> OutpointFilter for Runtime<D, K> {
    fn include_output(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        self.wallet()
            .coins()
            .any(|utxo| XChain::Bitcoin(utxo.outpoint) == output)
    }
}

pub struct ContractOutpointsFilter<'runtime, D: DescriptorRgb<K>, K> {
    pub contract_id: ContractId,
    pub filter: &'runtime Runtime<D, K>,
}

impl<'runtime, D: DescriptorRgb<K>, K> OutpointFilter for ContractOutpointsFilter<'runtime, D, K> {
    fn include_output(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        if !self.filter.include_output(output) {
            return false;
        }
        matches!(self.filter.stock.state_for_outpoints(self.contract_id, [output]), Ok(list) if !list.is_empty())
    }
}

#[cfg(feature = "serde")]
impl<D: DescriptorRgb<K>, K> Runtime<D, K>
where
    for<'de> D: serde::Serialize + serde::Deserialize<'de>,
    for<'de> bpwallet::WalletDescr<K, D>: serde::Serialize + serde::Deserialize<'de>,
{
    pub fn load_attach(
        mut stock_path: PathBuf,
        bprt: bpwallet::Runtime<D, K>,
    ) -> Result<Self, RuntimeError> {
        stock_path.push("stock.dat");

        let stock = Stock::load(&stock_path).or_else(|err| {
            if matches!(err, DeserializeError::Decode(DecodeError::Io(ref err)) if err.kind() == ErrorKind::NotFound) {
                eprint!("stock file is absent, creating a new one ... ");
                let stock = Stock::default();
                return Ok(stock)
            }
            eprintln!("stock file is damaged");
            Err(err)
        })?;

        Ok(Self {
            stock_path,
            stock,
            bprt,
        })
    }

    pub fn store(&mut self) {
        self.stock
            .store(&self.stock_path)
            .expect("unable to save stock");
        self.bprt.try_store().expect("unable to save wallet data");
    }
}

impl<D: DescriptorRgb<K>, K> Runtime<D, K> {
    pub fn wallet(&self) -> &Wallet<K, D> { self.bprt.wallet() }

    pub fn wallet_mut(&mut self) -> &mut Wallet<K, D> { self.bprt.wallet_mut() }

    pub fn attach(&mut self, bprt: bpwallet::Runtime<D, K>) { self.bprt = bprt }

    pub fn unload(self) {}

    pub fn network(&self) -> Network { self.bprt.network() }

    pub fn import_contract<R: ResolveHeight>(
        &mut self,
        contract: Contract,
        resolver: &mut R,
    ) -> Result<validation::Status, RuntimeError>
    where
        R::Error: 'static,
    {
        self.stock
            .import_contract(contract, resolver)
            .map_err(RuntimeError::from)
    }

    pub fn validate_transfer(
        &mut self,
        transfer: Transfer,
        resolver: &mut impl ResolveWitness,
    ) -> Result<Transfer, RuntimeError> {
        transfer
            .validate(resolver, self.network().is_testnet())
            .map_err(|invalid| invalid.validation_status().expect("just validated").clone())
            .map_err(RuntimeError::from)
    }

    pub fn accept_transfer<R: ResolveHeight>(
        &mut self,
        transfer: Transfer,
        resolver: &mut R,
        force: bool,
    ) -> Result<validation::Status, RuntimeError>
    where
        R::Error: 'static,
    {
        self.stock
            .accept_transfer(transfer, resolver, force)
            .map_err(RuntimeError::from)
    }
}
