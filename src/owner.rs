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

use std::convert::Infallible;

use amplify::Bytes32;
use bpstd::psbt::{PsbtConstructor, Utxo};
use bpstd::seals::WTxoSeal;
use bpstd::{Address, Keychain, Network, NormalIndex, Outpoint, ScriptPubkey, XpubDerivable};
use bpwallet::{
    Indexer, Layer2Empty, MayError, NoLayer2, Wallet, WalletCache, WalletData, WalletDescr,
};
use nonasync::persistence::{PersistenceError, PersistenceProvider};
use rgb::popls::bp::WalletProvider;
use rgb::{AuthToken, RgbSealDef};

use crate::descriptor::RgbDescr;
use crate::WalletUpdater;

// TODO: Use layer 2 supporting Lightning
#[derive(Wrapper, WrapperMut, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
pub struct Owner(
    pub Wallet<XpubDerivable, RgbDescr<XpubDerivable>, WalletCache<Layer2Empty>, NoLayer2>,
);

impl WalletProvider for Owner {
    fn noise_seed(&self) -> Bytes32 { self.noise() }

    fn has_utxo(&self, outpoint: Outpoint) -> bool { self.0.utxo(outpoint).is_some() }

    fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.0.utxos().map(|utxo| utxo.outpoint) }

    fn register_seal(&mut self, seal: WTxoSeal) {
        let _ = self.0.with_descriptor(|d| {
            d.add_seal(seal);
            Ok::<_, Infallible>(())
        });
    }

    fn resolve_seals(
        &self,
        seals: impl Iterator<Item = AuthToken>,
    ) -> impl Iterator<Item = WTxoSeal> {
        seals.flat_map(|auth| {
            self.0
                .descriptor()
                .seals()
                .filter(move |seal| seal.auth_token() == auth)
        })
    }

    fn next_address(&mut self) -> Address { self.0.next_address(Keychain::OUTER, true) }

    fn next_nonce(&mut self) -> u64 {
        let res = self
            .0
            .with_descriptor(|d| Ok::<_, Infallible>(d.next_nonce()));
        unsafe { res.unwrap_unchecked() }
    }
}

impl PsbtConstructor for Owner {
    type Key = XpubDerivable;
    type Descr = RgbDescr<XpubDerivable>;

    fn descriptor(&self) -> &Self::Descr { self.0.descriptor() }

    fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> { self.0.utxo(outpoint) }

    fn network(&self) -> Network { self.0.network() }

    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        self.0.next_derivation_index(keychain, shift)
    }
}

impl WalletUpdater for Owner {
    fn update<I: Indexer>(&mut self, indexer: &I) -> MayError<(), Vec<I::Error>> {
        self.0.update(indexer)
    }
}

impl Owner {
    pub fn create<P>(
        provider: P,
        descr: RgbDescr<XpubDerivable>,
        network: Network,
        autosave: bool,
    ) -> Result<Self, PersistenceError>
    where
        P: Clone
            + PersistenceProvider<WalletDescr<XpubDerivable, RgbDescr<XpubDerivable>, Layer2Empty>>
            + PersistenceProvider<WalletData<Layer2Empty>>
            + PersistenceProvider<WalletCache<Layer2Empty>>
            + PersistenceProvider<NoLayer2>
            + 'static,
    {
        let mut wallet = Wallet::new_layer1(descr, network);
        wallet.make_persistent(provider, autosave)?;
        Ok(Self(wallet))
    }

    pub fn load<P>(provider: P, autosave: bool) -> Result<Self, PersistenceError>
    where P: Clone
            + PersistenceProvider<WalletDescr<XpubDerivable, RgbDescr<XpubDerivable>, Layer2Empty>>
            + PersistenceProvider<WalletData<Layer2Empty>>
            + PersistenceProvider<WalletCache<Layer2Empty>>
            + PersistenceProvider<NoLayer2>
            + 'static {
        Wallet::load(provider, autosave).map(Owner)
    }
}
