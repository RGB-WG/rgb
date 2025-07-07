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

use amplify::Bytes32;
use bpstd::psbt::{PsbtConstructor, Utxo};
use bpstd::seals::WTxoSeal;
use bpstd::{
    Address, Derive, DeriveCompr, DeriveLegacy, DeriveSet, DeriveXOnly, Idx, Keychain, Network,
    NormalIndex, Outpoint, Sats, ScriptPubkey, Terminal, Txid, UnsignedTx, XpubDerivable,
};
use indexmap::IndexMap;
use rgb::popls::bp::WalletProvider;
use rgb::{AuthToken, RgbSealDef, WitnessStatus};

use crate::descriptor::RgbDescr;
use crate::Resolver;

pub trait UtxoSet {
    fn len(&self) -> usize;
    fn has(&self, outpoint: Outpoint) -> bool;
    fn get(&self, outpoint: Outpoint) -> Option<(Sats, Terminal)>;
    fn outpoints(&self) -> impl Iterator<Item = Outpoint>;

    fn clear(&mut self);
    fn extend(&mut self, set: impl IntoIterator<Item = Utxo>);
}

impl UtxoSet for IndexMap<Outpoint, (Sats, Terminal)> {
    #[inline]
    fn len(&self) -> usize { self.len() }
    #[inline]
    fn has(&self, outpoint: Outpoint) -> bool { self.contains_key(&outpoint) }
    #[inline]
    fn get(&self, outpoint: Outpoint) -> Option<(Sats, Terminal)> { self.get(&outpoint).copied() }
    #[inline]
    fn outpoints(&self) -> impl Iterator<Item = Outpoint> { self.keys().copied() }

    fn clear(&mut self) { self.clear() }

    fn extend(&mut self, set: impl IntoIterator<Item = Utxo>) {
        Extend::extend(
            self,
            set.into_iter()
                .map(|utxo| (utxo.outpoint, (utxo.value, utxo.terminal))),
        )
    }
}

#[derive(Clone)]
pub struct Owner<R, K = XpubDerivable, U = IndexMap<Outpoint, (Sats, Terminal)>>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    R: Resolver,
    U: UtxoSet,
{
    descriptor: RgbDescr<K>,
    network: Network,
    next_index: IndexMap<Keychain, NormalIndex>,
    utxos: U,
    resolver: R,
}

impl<R, K, U> WalletProvider for Owner<R, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    R: Resolver,
    U: UtxoSet,
{
    type SyncError = R::Error;

    fn has_utxo(&self, outpoint: Outpoint) -> bool { self.utxos.has(outpoint) }

    fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.utxos.outpoints() }

    fn sync_utxos(&mut self) -> Result<(), Self::SyncError> {
        self.utxos.clear();
        for keychain in self.descriptor.keychains() {
            let mut index = NormalIndex::ZERO;
            let last_index = self.next_index.get(&keychain).copied().unwrap_or_default();
            loop {
                let Some(to) = index.checked_add(20u16) else {
                    break;
                };

                let mut range = Vec::with_capacity(20);
                while index < to {
                    let terminal = Terminal::new(keychain, index);
                    let iter = self.descriptor.derive(keychain, index);

                    range.extend(iter.map(|d| (terminal, d.to_script_pubkey())));

                    if index.checked_inc_assign().is_none() {
                        break;
                    }
                }

                let set = self.resolver.resolve_utxos(range)?;
                let prev_len = self.utxos.len();
                self.utxos.extend(set);
                let next_len = self.utxos.len();
                if prev_len == next_len && index > last_index {
                    break;
                }

                index = to;
            }
        }
        Ok(())
    }

    fn register_seal(&mut self, seal: WTxoSeal) { self.descriptor.add_seal(seal); }

    fn resolve_seals(
        &self,
        seals: impl Iterator<Item = AuthToken>,
    ) -> impl Iterator<Item = WTxoSeal> {
        seals.flat_map(|auth| {
            self.descriptor
                .seals()
                .filter(move |seal| seal.auth_token() == auth)
        })
    }

    fn noise_seed(&self) -> Bytes32 { self.descriptor.noise() }

    fn next_address(&mut self) -> Address {
        let next = self.next_derivation_index(Keychain::OUTER, true);
        let spk = self
            .descriptor
            .derive(Keychain::OUTER, next)
            .next()
            .expect("at least one address must be derivable")
            .to_script_pubkey();
        Address::with(&spk, self.network).expect("invalid scriptpubkey derivation")
    }

    fn next_nonce(&mut self) -> u64 { self.descriptor.next_nonce() }

    fn txid_resolver(&self) -> impl Fn(Txid) -> Result<WitnessStatus, Self::SyncError> {
        |txid: Txid| self.resolver.resolve_tx_status(txid)
    }
}

impl<R, K, U> PsbtConstructor for Owner<R, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    R: Resolver,
    U: UtxoSet,
{
    type Key = K;
    type Descr = RgbDescr<K>;

    fn descriptor(&self) -> &Self::Descr { &self.descriptor }

    fn prev_tx(&self, txid: Txid) -> Option<UnsignedTx> { self.resolver.resolve_tx(txid).ok() }

    fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> {
        let (value, terminal) = self.utxos.get(outpoint)?;
        let utxo = Utxo { outpoint, value, terminal };
        let script = self
            .descriptor
            .derive(terminal.keychain, terminal.index)
            .next()
            .expect("unable to derive");
        Some((utxo, script.to_script_pubkey()))
    }

    fn network(&self) -> Network { self.network }

    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        let next = &mut self.next_index[&keychain.into()];
        if shift {
            next.saturating_inc_assign();
        }
        *next
    }
}
