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

use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use amplify::Bytes32;
use bpstd::psbt::{PsbtConstructor, Utxo};
use bpstd::seals::WTxoSeal;
use bpstd::{
    Address, Derive, DeriveCompr, DeriveLegacy, DeriveSet, DeriveXOnly, DescrId, Idx, Keychain,
    Network, NormalIndex, Outpoint, ScriptPubkey, Terminal, Tx, Txid, UnsignedTx, Vout,
    XpubDerivable,
};
use rgb::popls::bp::WalletProvider;
use rgb::{AuthToken, RgbSealDef, WitnessStatus};
use rgbdescr::RgbDescr;

use crate::resolvers::{Resolver, ResolverError};
use crate::{MemUtxos, UtxoSet};

pub trait OwnerProvider {
    type Key: DeriveSet<Legacy = Self::Key, Compr = Self::Key, XOnly = Self::Key>
        + DeriveLegacy
        + DeriveCompr
        + DeriveXOnly;
    type UtxoSet: UtxoSet;

    fn descriptor(&self) -> &RgbDescr<Self::Key>;
    fn utxos(&self) -> &Self::UtxoSet;
    fn descriptor_mut(&mut self) -> &mut RgbDescr<Self::Key>;
    fn utxos_mut(&mut self) -> &mut Self::UtxoSet;
}

#[derive(Clone)]
pub struct Holder<K = XpubDerivable, U = MemUtxos>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    U: UtxoSet,
{
    descriptor: RgbDescr<K>,
    utxos: U,
}

impl<K, U> Holder<K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    U: UtxoSet,
{
    #[inline]
    pub fn with_components(descriptor: RgbDescr<K>, utxos: U) -> Self { Self { descriptor, utxos } }
    #[inline]
    pub fn into_components(self) -> (RgbDescr<K>, U) { (self.descriptor, self.utxos) }
}

impl<K, U> OwnerProvider for Holder<K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    U: UtxoSet,
{
    type Key = K;
    type UtxoSet = U;

    #[inline]
    fn descriptor(&self) -> &RgbDescr<K> { &self.descriptor }
    #[inline]
    fn utxos(&self) -> &U { &self.utxos }
    #[inline]
    fn descriptor_mut(&mut self) -> &mut RgbDescr<K> { &mut self.descriptor }
    #[inline]
    fn utxos_mut(&mut self) -> &mut U { &mut self.utxos }
}

#[derive(Clone, Default)]
pub struct MultiHolder<K = XpubDerivable, U = MemUtxos>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    U: UtxoSet,
{
    current: Option<DescrId>,
    holders: HashMap<DescrId, Holder<K, U>>,
}

impl<K, U> MultiHolder<K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    U: UtxoSet,
{
    pub fn upsert(&mut self, id: DescrId, holder: Holder<K, U>) {
        self.holders.insert(id, holder);
        if self.current.is_none() {
            self.current = Some(id);
        }
    }

    pub fn remove(&mut self, id: DescrId) -> Option<Holder<K, U>> {
        if self.current == Some(id) {
            self.current = None;
        }
        self.holders.remove(&id)
    }

    pub fn switch(&mut self, new: DescrId) {
        self.current = Some(new);
        debug_assert!(self.holders.get(&new).is_some());
    }

    pub fn current(&self) -> &Holder<K, U> {
        &self.holders[&self
            .current
            .expect("current holder must be selected first with `MultiHolder::switch`")]
    }

    pub fn current_mut(&mut self) -> &mut Holder<K, U> {
        let current = self
            .current
            .expect("current holder must be selected first with `MultiHolder::switch`");
        self.holders
            .get_mut(&current)
            .expect("internal multiholder inconsistency")
    }
}

impl<K, U> OwnerProvider for MultiHolder<K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    U: UtxoSet,
{
    type Key = K;
    type UtxoSet = U;

    #[inline]
    fn descriptor(&self) -> &RgbDescr<K> { self.current().descriptor() }
    #[inline]
    fn utxos(&self) -> &U { self.current().utxos() }
    #[inline]
    fn descriptor_mut(&mut self) -> &mut RgbDescr<K> { self.current_mut().descriptor_mut() }
    #[inline]
    fn utxos_mut(&mut self) -> &mut U { self.current_mut().utxos_mut() }
}

/// Owner structure represents a holder of an RGB wallet, which keeps information of the wallet
/// descriptor and UTXO set. It doesn't know anything about RGB contracts, though (and that's why
/// it is not a full wallet) and is used as a component implementing [`WalletProvider`] inside
/// [`rgbstd::RgbWallet`] and [`crate::RgbRuntime`].
#[derive(Clone)]
pub struct Owner<R, O, K = XpubDerivable, U = MemUtxos>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    O: OwnerProvider<Key = K, UtxoSet = U>,
    R: Resolver,
    U: UtxoSet,
{
    network: Network,
    provider: O,
    resolver: R,
    _phantom: PhantomData<(K, U)>,
}

impl<R, O, K, U> Owner<R, O, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    O: OwnerProvider<Key = K, UtxoSet = U>,
    R: Resolver,
    U: UtxoSet,
{
    pub fn with_components(network: Network, provider: O, resolver: R) -> Self {
        Self { network, provider, resolver, _phantom: PhantomData }
    }

    pub fn into_components(self) -> (O, R) { (self.provider, self.resolver) }

    #[inline]
    pub fn network(&self) -> Network { self.network }
}

impl<R, O, K, U> WalletProvider for Owner<R, O, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    O: OwnerProvider<Key = K, UtxoSet = U>,
    R: Resolver,
    U: UtxoSet,
{
    type Error = ResolverError;

    fn has_utxo(&self, outpoint: Outpoint) -> bool { self.provider.utxos().has(outpoint) }

    fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.provider.utxos().outpoints() }

    #[cfg(not(feature = "async"))]
    fn update_utxos(&mut self) -> Result<(), Self::Error> {
        let mut new = set![];
        let mut not_found = self.provider.utxos().outpoints().collect::<HashSet<_>>();
        for keychain in self.provider.descriptor().keychains() {
            let mut index = NormalIndex::ZERO;
            let last_index = self.provider.utxos().next_index_noshift(keychain);
            loop {
                let Some(to) = index.checked_add(20u16) else {
                    break;
                };

                let mut range = Vec::with_capacity(20);
                while index < to {
                    let terminal = Terminal::new(keychain, index);
                    let iter = self.provider.descriptor().derive(keychain, index);

                    range.extend(iter.map(|d| (terminal, d.to_script_pubkey())));

                    if index.checked_inc_assign().is_none() {
                        break;
                    }
                }

                let set = self.resolver.resolve_utxos(range);
                let prev_len = self.provider.utxos().len();
                for utxo in set {
                    let utxo = utxo?;
                    not_found.remove(&utxo.outpoint);
                    if self.provider.utxos().has(utxo.outpoint) {
                        continue;
                    }
                    new.insert(utxo);
                }
                let next_len = self.provider.utxos().len();
                if prev_len == next_len && index > last_index {
                    break;
                }

                index = to;
            }
        }
        self.provider.utxos_mut().remove_all(not_found);
        self.provider.utxos_mut().insert_all(new);
        Ok(())
    }

    #[cfg(feature = "async")]
    async fn update_utxos_async(&mut self) -> Result<(), Self::Error> {
        let mut new = set![];
        let mut not_found = self.provider.utxos().outpoints().collect::<HashSet<_>>();
        for keychain in self.provider.descriptor().keychains() {
            let mut index = NormalIndex::ZERO;
            let last_index = self.provider.utxos().next_index_noshift(keychain);
            loop {
                let Some(to) = index.checked_add(20u16) else {
                    break;
                };

                let mut range = Vec::with_capacity(20);
                while index < to {
                    let terminal = Terminal::new(keychain, index);
                    let iter = self.provider.descriptor().derive(keychain, index);

                    range.extend(iter.map(|d| (terminal, d.to_script_pubkey())));

                    if index.checked_inc_assign().is_none() {
                        break;
                    }
                }

                let set = self.resolver.resolve_utxos_async(range).await;
                let prev_len = self.provider.utxos().len();
                for utxo in set {
                    let utxo = utxo?;
                    not_found.remove(&utxo.outpoint);
                    if self.provider.utxos().has(utxo.outpoint) {
                        continue;
                    }
                    new.insert(utxo);
                }
                let next_len = self.provider.utxos().len();
                if prev_len == next_len && index > last_index {
                    break;
                }

                index = to;
            }
        }
        self.provider.utxos_mut().remove_all_async(not_found).await;
        self.provider.utxos_mut().insert_all_async(new).await;
        Ok(())
    }

    fn register_seal(&mut self, seal: WTxoSeal) { self.provider.descriptor_mut().add_seal(seal); }

    fn resolve_seals(
        &self,
        seals: impl Iterator<Item = AuthToken>,
    ) -> impl Iterator<Item = WTxoSeal> {
        seals.flat_map(|auth| {
            self.provider
                .descriptor()
                .seals()
                .filter(move |seal| seal.auth_token() == auth)
        })
    }

    fn noise_seed(&self) -> Bytes32 { self.provider.descriptor().noise() }

    fn next_address(&mut self) -> Address {
        let next = self.next_derivation_index(Keychain::OUTER, true);
        let spk = self
            .provider
            .descriptor()
            .derive(Keychain::OUTER, next)
            .next()
            .expect("at least one address must be derivable")
            .to_script_pubkey();
        Address::with(&spk, self.network).expect("invalid scriptpubkey derivation")
    }

    fn next_nonce(&mut self) -> u64 { self.provider.descriptor_mut().next_nonce() }

    #[cfg(not(feature = "async"))]
    fn txid_resolver(&self) -> impl Fn(Txid) -> Result<WitnessStatus, Self::Error> {
        |txid: Txid| self.resolver.resolve_tx_status(txid)
    }

    #[cfg(feature = "async")]
    fn txid_resolver_async(&self) -> impl AsyncFn(Txid) -> Result<WitnessStatus, Self::Error> {
        |txid: Txid| self.resolver.resolve_tx_status_async(txid)
    }

    #[cfg(not(feature = "async"))]
    fn last_block_height(&self) -> Result<u64, Self::Error> { self.resolver.last_block_height() }

    #[cfg(feature = "async")]
    async fn last_block_height_async(&self) -> Result<u64, Self::Error> {
        self.resolver.last_block_height_async().await
    }

    #[cfg(not(feature = "async"))]
    fn broadcast(&mut self, tx: &Tx, change: Option<(Vout, u32, u32)>) -> Result<(), Self::Error> {
        self.resolver.broadcast(tx)?;

        for inp in &tx.inputs {
            self.provider.utxos_mut().remove(inp.prev_output);
        }
        if let Some((vout, keychain, index)) = change {
            let txid = tx.txid();
            let out = &tx.outputs[vout.into_usize()];
            let terminal = Terminal::new(
                Keychain::with(keychain as u8),
                NormalIndex::try_from_index(index).expect("invalid derivation index"),
            );
            self.provider
                .utxos_mut()
                .insert(Outpoint::new(txid, vout), out.value, terminal);
        }

        Ok(())
    }

    #[cfg(feature = "async")]
    async fn broadcast_async(
        &mut self,
        tx: &Tx,
        change: Option<(Vout, u32, u32)>,
    ) -> Result<(), Self::Error> {
        self.resolver.broadcast_async(tx).await?;

        for inp in &tx.inputs {
            self.provider
                .utxos_mut()
                .remove_async(inp.prev_output)
                .await;
        }
        if let Some((vout, keychain, index)) = change {
            let txid = tx.txid();
            let out = &tx.outputs[vout.into_usize()];
            let terminal = Terminal::new(
                Keychain::with(keychain as u8),
                NormalIndex::try_from_index(index).expect("invalid derivation index"),
            );
            self.provider
                .utxos_mut()
                .insert_async(Outpoint::new(txid, vout), out.value, terminal)
                .await;
        }

        Ok(())
    }
}

impl<R, O, K, U> PsbtConstructor for Owner<R, O, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    O: OwnerProvider<Key = K, UtxoSet = U>,
    R: Resolver,
    U: UtxoSet,
{
    type Key = K;
    type Descr = RgbDescr<K>;

    fn descriptor(&self) -> &Self::Descr { &self.provider.descriptor() }

    #[cfg(not(feature = "async"))]
    fn prev_tx(&self, txid: Txid) -> Option<UnsignedTx> {
        self.resolver.resolve_tx(txid).ok().flatten()
    }

    #[cfg(feature = "async")]
    fn prev_tx(&self, txid: Txid) -> Option<UnsignedTx> {
        use futures::executor::block_on;
        block_on(self.resolver.resolve_tx_async(txid))
            .ok()
            .flatten()
    }

    fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> {
        let (value, terminal) = self.provider.utxos().get(outpoint)?;
        let utxo = Utxo { outpoint, value, terminal };
        let script = self
            .provider
            .descriptor()
            .derive(terminal.keychain, terminal.index)
            .next()
            .expect("unable to derive");
        Some((utxo, script.to_script_pubkey()))
    }

    fn network(&self) -> Network { self.network }

    #[cfg(not(feature = "async"))]
    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        self.provider.utxos_mut().next_index(keychain, shift)
    }
    #[cfg(feature = "async")]
    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        use futures::executor::block_on;
        block_on(self.provider.utxos_mut().next_index_async(keychain, shift))
    }
}

#[cfg(feature = "fs")]
pub mod file {
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::{fs, io};

    use super::*;

    pub struct FileHolder {
        inner: Holder,
        path: PathBuf,
    }

    impl FileHolder {
        const DESCRIPTOR_FILENAME: &'static str = "descriptor.toml";
        const UTXO_FILENAME: &'static str = "utxo.toml";

        pub fn create(path: PathBuf, descriptor: RgbDescr) -> io::Result<Self> {
            fs::create_dir_all(&path)?;

            let mut file = fs::File::create_new(path.join(Self::DESCRIPTOR_FILENAME))?;
            let ser = toml::to_string(&descriptor)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            file.write_all(ser.as_bytes())?;

            let mut file = fs::File::create_new(path.join(Self::UTXO_FILENAME))?;
            let utxos = MemUtxos::default();
            let ser = toml::to_string(&utxos)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            file.write_all(ser.as_bytes())?;

            let inner = Holder::with_components(descriptor, utxos);
            Ok(Self { inner, path })
        }

        pub fn load(path: PathBuf) -> io::Result<Self> {
            let mut file = fs::File::open(path.join(Self::DESCRIPTOR_FILENAME))?;
            let mut deser = String::new();
            file.read_to_string(&mut deser)?;
            let descriptor: RgbDescr = toml::from_str(&deser)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

            let mut file = fs::File::open(path.join(Self::UTXO_FILENAME))?;
            let mut deser = String::new();
            file.read_to_string(&mut deser)?;
            let utxos: MemUtxos = toml::from_str(&deser)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

            let inner = Holder::with_components(descriptor, utxos);
            Ok(Self { inner, path })
        }

        pub fn save(&self) -> io::Result<()> {
            fs::create_dir_all(&self.path)?;

            let mut file = fs::File::create(self.path.join(Self::DESCRIPTOR_FILENAME))?;
            let ser = toml::to_string(&self.inner.descriptor())
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            file.write_all(ser.as_bytes())?;

            let mut file = fs::File::create(self.path.join(Self::UTXO_FILENAME))?;
            let ser = toml::to_string(&self.inner.utxos())
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            file.write_all(ser.as_bytes())?;

            Ok(())
        }

        pub fn path(&self) -> &Path { &self.path }
    }

    impl Drop for FileHolder {
        fn drop(&mut self) {
            if let Err(err) = self.save() {
                eprintln!("Error: unable to save wallet data. Details: {err}");
            }
        }
    }

    impl OwnerProvider for FileHolder {
        type Key = XpubDerivable;
        type UtxoSet = MemUtxos;
        #[inline]
        fn descriptor(&self) -> &RgbDescr<Self::Key> { self.inner.descriptor() }
        #[inline]
        fn utxos(&self) -> &Self::UtxoSet { self.inner.utxos() }
        #[inline]
        fn descriptor_mut(&mut self) -> &mut RgbDescr<Self::Key> { self.inner.descriptor_mut() }
        #[inline]
        fn utxos_mut(&mut self) -> &mut Self::UtxoSet { self.inner.utxos_mut() }
    }
}

#[cfg(all(test, not(feature = "async")))]
mod test {
    use bpstd::Sats;

    use super::*;

    fn setup() -> MemUtxos {
        let mut utxo = MemUtxos::default();
        let next = utxo.next_index(Keychain::OUTER, true);
        utxo.insert(Outpoint::coinbase(), Sats::from_btc(2), Terminal::new(Keychain::OUTER, next));
        utxo
    }

    #[test]
    fn mem_utxo_next_index() {
        let mut utxo = MemUtxos::default();
        let next = utxo.next_index(Keychain::OUTER, true);
        assert_eq!(next, NormalIndex::ZERO);

        let next = utxo.next_index(Keychain::OUTER, false);
        assert_eq!(next, NormalIndex::ONE);

        let next = utxo.next_index(Keychain::OUTER, false);
        assert_eq!(next, NormalIndex::ONE);
    }

    #[test]
    fn mem_utxo_insert() {
        let utxo = setup();
        assert!(utxo.has(Outpoint::coinbase()));
        assert_eq!(
            utxo.get(Outpoint::coinbase()),
            Some((Sats::from_btc(2), Terminal::new(Keychain::OUTER, NormalIndex::ZERO)))
        );
    }

    #[test]
    fn mem_utxo_serde() {
        let utxo = setup();
        let s = toml::to_string(&utxo).unwrap();
        assert_eq!(
            s,
            "[set]
\"0000000000000000000000000000000000000000000000000000000000000000:0\" = [200000000, \"&0/0\"]

[nextIndex]
0 = 1
"
        );

        let utxo2 = toml::from_str(&s).unwrap();
        assert_eq!(utxo, utxo2);
    }
}
