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

use std::error::Error;
use std::ops::DerefMut;
use std::path::Path;

use bp::{Outpoint, Txid};
use bpwallet::{Wallet, WalletDescr};
use psrgbt::PsbtConstructor;

use crate::DescriptorRgb;

pub trait Persisting {
    fn try_store(&self, path: &Path) -> Result<(), impl Error>;
}

pub trait WalletProvider<K>: PsbtConstructor
where Self::Descr: DescriptorRgb<K>
{
    fn descriptor_mut(&mut self) -> &mut Self::Descr;
    fn outpoints(&self) -> impl Iterator<Item = Outpoint>;
    fn txids(&self) -> impl Iterator<Item = Txid>;
}

#[cfg(feature = "fs")]
impl<K, D: DescriptorRgb<K>> Persisting for Wallet<K, D>
where
    for<'de> WalletDescr<K, D>: serde::Serialize + serde::Deserialize<'de>,
    for<'de> D: serde::Serialize + serde::Deserialize<'de>,
{
    fn try_store(&self, path: &Path) -> Result<(), impl Error> { self.store(path) }
}

impl<K, D: DescriptorRgb<K>> WalletProvider<K> for Wallet<K, D> {
    fn descriptor_mut(&mut self) -> &mut Self::Descr { self.deref_mut() }

    fn outpoints(&self) -> impl Iterator<Item = Outpoint> { self.coins().map(|coin| coin.outpoint) }

    fn txids(&self) -> impl Iterator<Item = Txid> { self.transactions().keys().copied() }
}
