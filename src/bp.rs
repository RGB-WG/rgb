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

use std::collections::HashMap;
use std::convert::Infallible;

use amplify::Bytes32;
use bpstd::psbt::{PsbtConstructor, Utxo};
use bpstd::seals::WTxoSeal;
use bpstd::{
    Address, Derive, DeriveCompr, DeriveLegacy, DeriveSet, DeriveXOnly, Idx, Keychain, Network,
    NormalIndex, Outpoint, Sats, ScriptPubkey, Terminal, Txid, UnsignedTx,
};
use rgb::popls::bp::WalletProvider;
use rgb::{AuthToken, RgbSealDef, WitnessStatus};

use crate::descriptor::RgbDescr;

#[derive(Clone)]
pub struct Owner<
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
> {
    descriptor: RgbDescr<K>,
    network: Network,
    next_index: HashMap<Keychain, NormalIndex>,
    utxos: HashMap<Outpoint, (Sats, Terminal)>,
    // TODO: Add indexer
}

impl<K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly>
    Owner<K>
{
    fn resolve_tx(&self, txid: Txid) -> Result<UnsignedTx, Infallible> { todo!() }
    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, Infallible> { todo!() }
}

impl<K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly>
    WalletProvider for Owner<K>
{
    type SyncError = Infallible;

    fn has_utxo(&self, outpoint: Outpoint) -> bool { self.utxos.contains_key(&outpoint) }

    fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.utxos.keys().copied() }

    fn sync_utxos(&mut self) -> Result<(), Self::SyncError> { todo!() }

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
        |txid: Txid| self.resolve_tx_status(txid)
    }
}

impl<K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly>
    PsbtConstructor for Owner<K>
{
    type Key = K;
    type Descr = RgbDescr<K>;

    fn descriptor(&self) -> &Self::Descr { &self.descriptor }

    fn prev_tx(&self, txid: Txid) -> Option<UnsignedTx> { self.resolve_tx(txid).ok() }

    fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> {
        let (value, terminal) = self.utxos.get(&outpoint).copied()?;
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
        let next = self
            .next_index
            .get_mut(&keychain.into())
            .expect("must be present");
        if shift {
            next.saturating_inc_assign();
            // TODO: Mark dirty
        }
        *next
    }
}
