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
use std::error::Error;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use bpstd::XpubDerivable;
use bpwallet::{StoreError, Wallet, WalletDescr};
use psrgbt::{Psbt, PsbtMeta};
use rgbstd::containers::Transfer;
use rgbstd::interface::{AmountChange, IfaceOp, IfaceRef};
use rgbstd::persistence::fs::StoreFs;
use rgbstd::persistence::{
    IndexProvider, MemIndex, MemStash, MemState, StashProvider, StateProvider, Stock,
};

use super::{
    CompletionError, CompositionError, ContractId, DescriptorRgb, PayError, TransferParams,
    WalletError, WalletProvider, WalletStock, XWitnessId,
};
use crate::invoice::RgbInvoice;

pub trait Store {
    type Err: Error;
    fn store(&self, path: impl AsRef<Path>) -> Result<(), Self::Err>;
}

impl<K, D: DescriptorRgb<K>> Store for Wallet<K, D>
where
    for<'de> WalletDescr<K, D>: serde::Serialize + serde::Deserialize<'de>,
    for<'de> D: serde::Serialize + serde::Deserialize<'de>,
{
    type Err = StoreError;
    fn store(&self, path: impl AsRef<Path>) -> Result<(), Self::Err> { self.store(path.as_ref()) }
}

#[derive(Getters)]
pub struct StoredStock<
    S: StashProvider = MemStash,
    H: StateProvider = MemState,
    P: IndexProvider = MemIndex,
> where
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    stock_path: PathBuf,
    stock: Stock<S, H, P>,
    #[getter(prefix = "is_")]
    dirty: bool,
}

impl<S: StashProvider, H: StateProvider, P: IndexProvider> Deref for StoredStock<S, H, P>
where
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    type Target = Stock<S, H, P>;

    fn deref(&self) -> &Self::Target { &self.stock }
}

impl<S: StashProvider, H: StateProvider, P: IndexProvider> DerefMut for StoredStock<S, H, P>
where
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.dirty = true;
        &mut self.stock
    }
}

impl<S: StashProvider, H: StateProvider, P: IndexProvider> StoredStock<S, H, P>
where
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    pub fn attach(path: PathBuf, stock: Stock<S, H, P>) -> Self {
        Self {
            stock_path: path,
            stock,
            dirty: false,
        }
    }

    pub fn store(&self) {
        if self.dirty {
            self.stock
                .store(&self.stock_path)
                .expect("error saving data");
        }
    }
}

impl<S: StashProvider, H: StateProvider, P: IndexProvider> Drop for StoredStock<S, H, P>
where
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    fn drop(&mut self) { self.store() }
}

#[derive(Getters)]
pub struct StoredWallet<
    W: WalletProvider<K>,
    K = XpubDerivable,
    S: StashProvider = MemStash,
    H: StateProvider = MemState,
    P: IndexProvider = MemIndex,
> where
    W::Descr: DescriptorRgb<K>,
    W: Store,
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    stock_path: PathBuf,
    stock: Stock<S, H, P>,
    wallet: W,
    #[getter(prefix = "is_")]
    stock_dirty: bool,
    #[getter(prefix = "is_")]
    wallet_dirty: bool,
    #[getter(skip)]
    _phantom: PhantomData<K>,
}

impl<K, W: WalletProvider<K>, S: StashProvider, H: StateProvider, P: IndexProvider>
    StoredWallet<W, K, S, H, P>
where
    W::Descr: DescriptorRgb<K>,
    W: Store,
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    pub fn attach(path: PathBuf, stock: Stock<S, H, P>, wallet: W) -> Self {
        Self {
            stock_path: path,
            stock,
            wallet,
            stock_dirty: false,
            wallet_dirty: false,
            _phantom: PhantomData,
        }
    }

    pub fn stock_mut(&mut self) -> &mut Stock<S, H, P> {
        self.stock_dirty = true;
        &mut self.stock
    }

    pub fn wallet_mut(&mut self) -> &mut W {
        self.wallet_dirty = true;
        &mut self.wallet
    }

    #[allow(clippy::result_large_err)]
    pub fn fungible_history(
        &self,
        contract_id: ContractId,
        iface: impl Into<IfaceRef>,
    ) -> Result<HashMap<XWitnessId, IfaceOp<AmountChange>>, WalletError> {
        self.stock
            .fungible_history(&self.wallet, contract_id, iface)
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

    pub fn store(&self) {
        let r1 = if self.wallet_dirty {
            self.stock
                .store(&self.stock_path)
                .map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        let r2 = if self.wallet_dirty {
            self.wallet
                .store(&self.stock_path)
                .map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        r1.and(r2).expect("error saving data");
    }
}

impl<K, W: WalletProvider<K>, S: StashProvider, H: StateProvider, P: IndexProvider> Drop
    for StoredWallet<W, K, S, H, P>
where
    W::Descr: DescriptorRgb<K>,
    W: Store,
    S: StoreFs,
    H: StoreFs,
    P: StoreFs,
{
    fn drop(&mut self) { self.store() }
}
