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

use std::collections::HashMap;
use std::marker::PhantomData;
#[cfg(feature = "fs")]
use std::path::PathBuf;

use bpstd::XpubDerivable;
#[cfg(feature = "fs")]
use bpwallet::Wallet;
#[cfg(feature = "fs")]
use bpwallet::fs::FsTextStore;
#[cfg(not(target_arch = "wasm32"))]
use nonasync::persistence::PersistenceProvider;
use psrgbt::{Psbt, PsbtMeta};
use rgbstd::containers::Transfer;
use rgbstd::interface::{AmountChange, IfaceOp, IfaceRef};
#[cfg(feature = "fs")]
use rgbstd::persistence::fs::FsBinStore;
use rgbstd::persistence::{
    IndexProvider, MemIndex, MemStash, MemState, StashProvider, StateProvider, Stock,
};

use super::{
    CompletionError, CompositionError, ContractId, DescriptorRgb, HistoryError, PayError,
    TransferParams, WalletError, WalletProvider, XWitnessId,
};
use crate::invoice::RgbInvoice;

#[derive(Getters)]
pub struct RgbWallet<
    W: WalletProvider<K>,
    K = XpubDerivable,
    S: StashProvider = MemStash,
    H: StateProvider = MemState,
    P: IndexProvider = MemIndex,
> where W::Descr: DescriptorRgb<K>
{
    stock: Stock<S, H, P>,
    wallet: W,
    #[getter(skip)]
    _phantom: PhantomData<K>,
}

#[cfg(feature = "fs")]
impl<K, D: DescriptorRgb<K>, S: StashProvider, H: StateProvider, P: IndexProvider>
    RgbWallet<Wallet<K, D>, K, S, H, P>
{
    #[allow(clippy::result_large_err)]
    pub fn load(
        stock_path: PathBuf,
        wallet_path: PathBuf,
        autosave: bool,
    ) -> Result<Self, WalletError>
    where
        D: serde::Serialize + for<'de> serde::Deserialize<'de>,
        FsBinStore: PersistenceProvider<S>,
        FsBinStore: PersistenceProvider<H>,
        FsBinStore: PersistenceProvider<P>,
    {
        let provider = FsBinStore::new(stock_path)?;
        let stock = Stock::load(provider, autosave).map_err(WalletError::StockPersist)?;
        let provider = FsTextStore::new(wallet_path)?;
        let wallet = Wallet::load(provider, autosave).map_err(WalletError::WalletPersist)?;
        Ok(Self {
            wallet,
            stock,
            _phantom: PhantomData,
        })
    }
}

impl<K, W: WalletProvider<K>, S: StashProvider, H: StateProvider, P: IndexProvider>
    RgbWallet<W, K, S, H, P>
where W::Descr: DescriptorRgb<K>
{
    pub fn new(stock: Stock<S, H, P>, wallet: W) -> Self {
        Self {
            stock,
            wallet,
            _phantom: PhantomData,
        }
    }

    pub fn stock_mut(&mut self) -> &mut Stock<S, H, P> { &mut self.stock }

    pub fn wallet_mut(&mut self) -> &mut W { &mut self.wallet }

    #[allow(clippy::result_large_err)]
    pub fn fungible_history(
        &self,
        contract_id: ContractId,
        iface: impl Into<IfaceRef>,
    ) -> Result<HashMap<XWitnessId, IfaceOp<AmountChange>>, WalletError> {
        let wallet = &self.wallet;
        let iref = iface.into();
        let iface = self.stock.iface(iref.clone()).map_err(|e| e.to_string())?;
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
        let contract = self
            .stock
            .contract_iface(contract_id, iref)
            .map_err(|e| e.to_string())?;
        Ok(contract
            .fungible_ops::<AmountChange>(state_name, wallet.filter())
            .map_err(|e| e.to_string())?)
    }

    #[allow(clippy::result_large_err)]
    pub fn pay(
        &mut self,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta, Transfer), PayError> {
        self.wallet.pay(&mut self.stock, invoice, params)
    }

    #[allow(clippy::result_large_err)]
    pub fn construct_psbt(
        &mut self,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), CompositionError> {
        self.wallet.construct_psbt_rgb(&self.stock, invoice, params)
    }

    #[allow(clippy::result_large_err)]
    pub fn transfer(
        &mut self,
        invoice: &RgbInvoice,
        psbt: &mut Psbt,
    ) -> Result<Transfer, CompletionError> {
        self.wallet.transfer(&mut self.stock, invoice, psbt)
    }
}
