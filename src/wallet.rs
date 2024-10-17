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

use std::marker::PhantomData;
#[cfg(feature = "fs")]
use std::path::PathBuf;

use bpstd::XpubDerivable;
#[cfg(feature = "fs")]
use bpwallet::Wallet;
#[cfg(feature = "fs")]
use bpwallet::fs::FsTextStore;
use bpwallet::{Layer2, NoLayer2};
#[cfg(not(target_arch = "wasm32"))]
use nonasync::persistence::PersistenceProvider;
use psrgbt::{Psbt, PsbtMeta};
use rgbstd::containers::Transfer;
use rgbstd::interface::{ContractOp, IfaceRef};
#[cfg(feature = "fs")]
use rgbstd::persistence::fs::FsBinStore;
use rgbstd::persistence::{
    ContractIfaceError, IndexProvider, MemIndex, MemStash, MemState, StashProvider, StateProvider,
    Stock, StockError,
};

use super::{
    CompletionError, CompositionError, ContractId, DescriptorRgb, PayError, TransferParams,
    WalletError, WalletProvider,
};
use crate::invoice::RgbInvoice;

#[derive(Getters)]
pub struct RgbWallet<
    W: WalletProvider<K, L2>,
    K = XpubDerivable,
    S: StashProvider = MemStash,
    H: StateProvider = MemState,
    P: IndexProvider = MemIndex,
    L2: Layer2 = NoLayer2,
> where W::Descr: DescriptorRgb<K>
{
    stock: Stock<S, H, P>,
    wallet: W,
    #[getter(skip)]
    _key_phantom: PhantomData<K>,
    #[getter(skip)]
    _layer2_phantom: PhantomData<L2>,
}

#[cfg(feature = "fs")]
impl<K, D: DescriptorRgb<K>, S: StashProvider, H: StateProvider, P: IndexProvider, L2: Layer2>
    RgbWallet<Wallet<K, D, L2>, K, S, H, P, L2>
{
    #[allow(clippy::result_large_err)]
    pub fn load(
        stock_path: PathBuf,
        wallet_path: PathBuf,
        autosave: bool,
    ) -> Result<Self, WalletError>
    where
        D: serde::Serialize + for<'de> serde::Deserialize<'de>,
        L2::Descr: serde::Serialize + for<'de> serde::Deserialize<'de>,
        L2::Data: serde::Serialize + for<'de> serde::Deserialize<'de>,
        L2::Cache: serde::Serialize + for<'de> serde::Deserialize<'de>,
        FsBinStore: PersistenceProvider<S>,
        FsBinStore: PersistenceProvider<H>,
        FsBinStore: PersistenceProvider<P>,
        FsTextStore: PersistenceProvider<L2>,
    {
        use nonasync::persistence::PersistenceError;
        let provider = FsBinStore::new(stock_path)
            .map_err(|e| WalletError::StockPersist(PersistenceError::with(e)))?;
        let stock = Stock::load(provider, autosave).map_err(WalletError::StockPersist)?;
        let provider = FsTextStore::new(wallet_path)
            .map_err(|e| WalletError::WalletPersist(PersistenceError::with(e)))?;
        let wallet = Wallet::load(provider, autosave).map_err(WalletError::WalletPersist)?;
        Ok(Self {
            wallet,
            stock,
            _key_phantom: PhantomData,
            _layer2_phantom: PhantomData,
        })
    }
}

impl<K, W: WalletProvider<K, L2>, S: StashProvider, H: StateProvider, P: IndexProvider, L2: Layer2>
    RgbWallet<W, K, S, H, P, L2>
where W::Descr: DescriptorRgb<K>
{
    pub fn new(stock: Stock<S, H, P>, wallet: W) -> Self {
        Self {
            stock,
            wallet,
            _key_phantom: PhantomData,
            _layer2_phantom: PhantomData,
        }
    }

    pub fn stock_mut(&mut self) -> &mut Stock<S, H, P> { &mut self.stock }

    pub fn wallet_mut(&mut self) -> &mut W { &mut self.wallet }

    pub fn history(
        &self,
        contract_id: ContractId,
        iface: impl Into<IfaceRef>,
    ) -> Result<Vec<ContractOp>, StockError<S, H, P, ContractIfaceError>> {
        let contract = self.stock.contract_iface(contract_id, iface.into())?;
        let wallet = &self.wallet;
        Ok(contract.history(wallet.filter_outpoints(), wallet.filter_witnesses()))
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
        invoice: RgbInvoice,
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
