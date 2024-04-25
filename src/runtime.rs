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

use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use amplify::IoError;
use bpstd::{Network, XpubDerivable};
use bpwallet::Wallet;
use rgbstd::containers::LoadError;
use rgbstd::interface::{
    AmountChange, BuilderError, ContractError, IfaceOp, IfaceRef, OutpointFilter, WitnessFilter,
};
use rgbstd::persistence::fs::{LoadFs, StoreFs};
use rgbstd::persistence::{ContractIfaceError, Stock, StockError, StockErrorAll, StockErrorMem};
use rgbstd::validation::{self};
use rgbstd::{AssignmentWitness, ContractId, XChain, XOutpoint, XWitnessId};
use strict_types::encoding::{DeserializeError, Ident, SerializeError};

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
    Builder(BuilderError),

    #[from]
    History(HistoryError),

    #[from]
    Contract(ContractError),

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
    InvalidId(baid64::Baid64ParseError),

    /// the contract source doesn't fit requirements imposed by the used schema.
    ///
    /// {0}
    #[display(doc_comments)]
    IncompleteContract(validation::Status),

    #[from]
    #[from(bpwallet::LoadError)]
    Bp(bpwallet::RuntimeError),

    /// resolver error: {0}
    #[cfg(any(feature = "electrum", feature = "esplora_blocking"))]
    #[from]
    #[display(doc_comments)]
    ResolverError(crate::AnyResolverError),

    #[from]
    #[from(StockError)]
    #[from(StockErrorMem<ContractIfaceError>)]
    #[display(inner)]
    Stock(StockErrorAll),

    #[cfg(feature = "serde_yaml")]
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
    // TODO: Parametrize by the stock
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
    fn include_outpoint(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        self.wallet()
            .coins()
            .any(|utxo| XChain::Bitcoin(utxo.outpoint) == *output)
    }
}

impl<D: DescriptorRgb<K>, K> WitnessFilter for Runtime<D, K> {
    fn include_witness(&self, witness: impl Into<AssignmentWitness>) -> bool {
        let witness = witness.into();
        self.wallet()
            .transactions()
            .keys()
            .any(|txid| AssignmentWitness::Present(XWitnessId::Bitcoin(*txid)) == witness)
    }
}

pub struct ContractOutpointsFilter<'runtime, D: DescriptorRgb<K>, K> {
    pub contract_id: ContractId,
    pub filter: &'runtime Runtime<D, K>,
}

impl<'runtime, D: DescriptorRgb<K>, K> OutpointFilter for ContractOutpointsFilter<'runtime, D, K> {
    fn include_outpoint(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        if !self.filter.include_outpoint(output) {
            return false;
        }
        matches!(self.filter.stock.contract_assignments_for(self.contract_id, [output]), Ok(list) if !list.is_empty())
    }
}

#[cfg(feature = "serde")]
impl<D: DescriptorRgb<K>, K> Runtime<D, K>
where
    for<'de> D: serde::Serialize + serde::Deserialize<'de>,
    for<'de> bpwallet::WalletDescr<K, D>: serde::Serialize + serde::Deserialize<'de>,
{
    pub fn load_walletless(stock_path: &PathBuf) -> Result<Stock, RuntimeError> {
        use std::io::ErrorKind;

        use strict_types::encoding::DecodeError;

        Stock::load(stock_path).map_err(RuntimeError::from).or_else(|err| {
            if matches!(err, RuntimeError::Deserialize(DeserializeError::Decode(DecodeError::Io(ref err))) if err.kind() == ErrorKind::NotFound) {
                #[cfg(feature = "log")]
                eprint!("stock file is absent, creating a new one ... ");
                let stock = Stock::default();
                std::fs::create_dir_all(stock_path)?;
                stock.store(stock_path)?;
                eprintln!("success");
                return Ok(stock)
            }
            eprintln!("stock file is damaged");
            Err(err)
        })
    }

    pub fn load_attach(
        stock_path: PathBuf,
        bprt: bpwallet::Runtime<D, K>,
    ) -> Result<Self, RuntimeError> {
        let stock = Self::load_walletless(&stock_path)?;
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

    pub fn into_stock(self) -> Stock { self.stock }
}

impl<D: DescriptorRgb<K>, K> Runtime<D, K> {
    pub fn wallet(&self) -> &Wallet<K, D> { self.bprt.wallet() }

    pub fn wallet_mut(&mut self) -> &mut Wallet<K, D> { self.bprt.wallet_mut() }

    pub fn attach(&mut self, bprt: bpwallet::Runtime<D, K>) { self.bprt = bprt }

    pub fn network(&self) -> Network { self.bprt.network() }

    // TODO: Integrate into BP Wallet `TxRow` as L2 and provide transactional info
    pub fn fungible_history(
        &self,
        contract_id: ContractId,
        iface: impl Into<IfaceRef>,
    ) -> Result<HashMap<XWitnessId, IfaceOp<AmountChange>>, RuntimeError> {
        let iref = iface.into();
        let iface = self.stock.iface(iref.clone())?;
        let default_op = iface
            .default_operation
            .as_ref()
            .ok_or(HistoryError::NoDefaultOp)?;
        let state_name = iface
            .transitions
            .get(default_op)
            .ok_or(HistoryError::DefaultOpNotTransition)?
            .default_assignment
            .as_ref()
            .ok_or(HistoryError::NoDefaultAssignment)?
            .clone();
        let contract = self.stock.contract_iface(contract_id, iref)?;
        contract
            .fungible_ops::<AmountChange>(state_name, self, self)
            .map_err(RuntimeError::from)
    }
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum HistoryError {
    /// interface doesn't define default operation
    NoDefaultOp,
    /// default operation defined by the interface is not a state transition
    DefaultOpNotTransition,
    /// interface doesn't define default fungible state
    NoDefaultAssignment,
}
