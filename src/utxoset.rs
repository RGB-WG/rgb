// Wallet Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
// Copyright (C) 2025 RGB Consortium, Switzerland.
// Copyright (C) 2019-2025 Dr Maxim Orlovsky.
// All rights under the above copyrights are reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use bpstd::{Idx, Keychain, NormalIndex, Outpoint, Sats, Terminal};
use indexmap::IndexMap;

#[allow(clippy::len_without_is_empty)]
pub trait UtxoSet {
    fn len(&self) -> usize;
    fn has(&self, outpoint: Outpoint) -> bool;
    fn get(&self, outpoint: Outpoint) -> Option<(Sats, Terminal)>;

    #[cfg(not(feature = "async"))]
    fn insert(&mut self, outpoint: Outpoint, value: Sats, terminal: Terminal);
    #[cfg(feature = "async")]
    async fn insert_async(&mut self, outpoint: Outpoint, value: Sats, terminal: Terminal);

    #[cfg(not(feature = "async"))]
    fn clear(&mut self);
    #[cfg(feature = "async")]
    async fn clear_async(&mut self);

    #[cfg(not(feature = "async"))]
    fn remove(&mut self, outpoint: Outpoint) -> Option<(Sats, Terminal)>;
    #[cfg(feature = "async")]
    async fn remove_async(&mut self, outpoint: Outpoint) -> Option<(Sats, Terminal)>;

    fn outpoints(&self) -> impl Iterator<Item = Outpoint>;

    #[cfg(not(feature = "async"))]
    fn next_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex;
    #[cfg(feature = "async")]
    async fn next_index_async(&mut self, keychain: impl Into<Keychain>, shift: bool)
        -> NormalIndex;
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "camelCase"))]
pub struct MemUtxos {
    set: IndexMap<Outpoint, (Sats, Terminal)>,
    next_index: IndexMap<Keychain, NormalIndex>,
}

impl UtxoSet for MemUtxos {
    #[inline]
    fn len(&self) -> usize { self.set.len() }
    #[inline]
    fn has(&self, outpoint: Outpoint) -> bool { self.set.contains_key(&outpoint) }
    #[inline]
    fn get(&self, outpoint: Outpoint) -> Option<(Sats, Terminal)> {
        self.set.get(&outpoint).copied()
    }

    #[inline]
    #[cfg(not(feature = "async"))]
    fn insert(&mut self, outpoint: Outpoint, value: Sats, terminal: Terminal) {
        self.set.insert(outpoint, (value, terminal));
    }
    #[inline]
    #[cfg(feature = "async")]
    async fn insert_async(&mut self, outpoint: Outpoint, value: Sats, terminal: Terminal) {
        self.set.insert(outpoint, (value, terminal));
    }

    #[inline]
    #[cfg(not(feature = "async"))]
    fn clear(&mut self) { self.set.clear() }
    #[inline]
    #[cfg(feature = "async")]
    async fn clear_async(&mut self) { self.set.clear() }

    #[inline]
    #[cfg(not(feature = "async"))]
    fn remove(&mut self, outpoint: Outpoint) -> Option<(Sats, Terminal)> {
        self.set.shift_remove(&outpoint)
    }
    #[inline]
    #[cfg(feature = "async")]
    async fn remove_async(&mut self, outpoint: Outpoint) -> Option<(Sats, Terminal)> {
        self.set.shift_remove(&outpoint)
    }

    #[inline]
    fn outpoints(&self) -> impl Iterator<Item = Outpoint> { self.set.keys().copied() }

    #[cfg(not(feature = "async"))]
    fn next_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        let index = self.next_index.entry(keychain.into()).or_default();
        let next = *index;
        if shift {
            index.saturating_inc_assign();
        }
        next
    }
    #[inline]
    #[cfg(feature = "async")]
    async fn next_index_async(
        &mut self,
        keychain: impl Into<Keychain>,
        shift: bool,
    ) -> NormalIndex {
        let index = self.next_index.entry(keychain.into()).or_default();
        let next = *index;
        if shift {
            index.saturating_inc_assign();
        }
        next
    }
}
