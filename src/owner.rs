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
    NormalIndex, Outpoint, Sats, ScriptPubkey, Terminal, Tx, Txid, UnsignedTx, Vout, XpubDerivable,
};
use indexmap::IndexMap;
use rgb::popls::bp::WalletProvider;
use rgb::{AuthToken, RgbSealDef, WitnessStatus};

use crate::descriptor::RgbDescr;
use crate::resolvers::{Resolver, ResolverError};

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

/// Owner structure represents a holder of an RGB wallet, which keeps information of the wallet
/// descriptor and UTXO set. It doesn't know anything about RGB contracts, though (and that's why
/// it is not a full wallet) and is used as a component implementing [`WalletProvider`] inside
/// [`rgbstd::RgbWallet`] and [`crate::RgbRuntime`].
#[derive(Clone)]
pub struct Owner<R, K = XpubDerivable, U = MemUtxos>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    R: Resolver,
    U: UtxoSet,
{
    network: Network,
    descriptor: RgbDescr<K>,
    utxos: U,
    resolver: R,
}

impl<R, K, U> Owner<R, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    R: Resolver,
    U: UtxoSet,
{
    pub fn with_components(
        network: Network,
        descriptor: RgbDescr<K>,
        resolver: R,
        utxos: U,
    ) -> Self {
        Self { network, descriptor, utxos, resolver }
    }

    pub fn into_components(self) -> (RgbDescr<K>, R, U) {
        (self.descriptor, self.resolver, self.utxos)
    }

    #[inline]
    pub fn network(&self) -> Network { self.network }
}

impl<R, K, U> WalletProvider for Owner<R, K, U>
where
    K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly,
    R: Resolver,
    U: UtxoSet,
{
    type Error = ResolverError;

    fn has_utxo(&self, outpoint: Outpoint) -> bool { self.utxos.has(outpoint) }

    fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.utxos.outpoints() }

    #[cfg(not(feature = "async"))]
    fn update_utxos(&mut self) -> Result<(), Self::Error> {
        self.utxos.clear();
        for keychain in self.descriptor.keychains() {
            let mut index = NormalIndex::ZERO;
            let last_index = self.utxos.next_index(keychain, false);
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

                let set = self.resolver.resolve_utxos(range);
                let prev_len = self.utxos.len();
                for utxo in set {
                    let utxo = utxo?;
                    self.utxos.insert(utxo.outpoint, utxo.value, utxo.terminal);
                }
                let next_len = self.utxos.len();
                if prev_len == next_len && index > last_index {
                    break;
                }

                index = to;
            }
        }
        Ok(())
    }

    #[cfg(feature = "async")]
    async fn update_utxos_async(&mut self) -> Result<(), Self::Error> {
        self.utxos.clear_async().await;
        for keychain in self.descriptor.keychains() {
            let mut index = NormalIndex::ZERO;
            let last_index = self.utxos.next_index_async(keychain, false).await;
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

                let set = self.resolver.resolve_utxos_async(range).await;
                let prev_len = self.utxos.len();
                for utxo in set {
                    let utxo = utxo?;
                    self.utxos
                        .insert_async(utxo.outpoint, utxo.value, utxo.terminal)
                        .await;
                }
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
            self.utxos.remove(inp.prev_output);
        }
        if let Some((vout, keychain, index)) = change {
            let txid = tx.txid();
            let out = &tx.outputs[vout.into_usize()];
            let terminal = Terminal::new(
                Keychain::with(keychain as u8),
                NormalIndex::try_from_index(index).expect("invalid derivation index"),
            );
            self.utxos
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
            self.utxos.remove_async(inp.prev_output).await;
        }
        if let Some((vout, keychain, index)) = change {
            let txid = tx.txid();
            let out = &tx.outputs[vout.into_usize()];
            let terminal = Terminal::new(
                Keychain::with(keychain as u8),
                NormalIndex::try_from_index(index).expect("invalid derivation index"),
            );
            self.utxos
                .insert_async(Outpoint::new(txid, vout), out.value, terminal)
                .await;
        }

        Ok(())
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

    #[cfg(not(feature = "async"))]
    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        self.utxos.next_index(keychain, shift)
    }
    #[cfg(feature = "async")]
    fn next_derivation_index(&mut self, keychain: impl Into<Keychain>, shift: bool) -> NormalIndex {
        use futures::executor::block_on;
        block_on(self.utxos.next_index_async(keychain, shift))
    }
}

#[cfg(feature = "fs")]
pub mod file {
    use std::io::{Read, Write};
    use std::ops::{Deref, DerefMut};
    use std::path::{Path, PathBuf};
    use std::{fs, io};

    use super::*;

    pub struct FileOwner<R: Resolver> {
        owner: Owner<R>,
        path: PathBuf,
    }

    impl<R: Resolver> Deref for FileOwner<R> {
        type Target = Owner<R>;

        fn deref(&self) -> &Self::Target { &self.owner }
    }

    impl<R: Resolver> DerefMut for FileOwner<R> {
        fn deref_mut(&mut self) -> &mut Self::Target { &mut self.owner }
    }

    impl<R: Resolver> FileOwner<R> {
        const DESCRIPTOR_FILENAME: &'static str = "descriptor.toml";
        const UTXO_FILENAME: &'static str = "utxo.toml";

        pub fn create(
            path: PathBuf,
            network: Network,
            descriptor: RgbDescr,
            resolver: R,
        ) -> io::Result<Self> {
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

            let owner = Owner::with_components(network, descriptor, resolver, utxos);
            Ok(Self { owner, path })
        }

        pub fn load(path: PathBuf, network: Network, resolver: R) -> io::Result<Self> {
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

            let owner = Owner::with_components(network, descriptor, resolver, utxos);
            Ok(Self { owner, path })
        }

        pub fn save(&self) -> io::Result<()> {
            fs::create_dir_all(&self.path)?;

            let mut file = fs::File::create(self.path.join(Self::DESCRIPTOR_FILENAME))?;
            let ser = toml::to_string(&self.descriptor)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            file.write_all(ser.as_bytes())?;

            let mut file = fs::File::create(self.path.join(Self::UTXO_FILENAME))?;
            let ser = toml::to_string(&self.utxos)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            file.write_all(ser.as_bytes())?;

            Ok(())
        }

        pub fn path(&self) -> &Path { &self.path }
    }

    impl<R: Resolver> Drop for FileOwner<R> {
        fn drop(&mut self) {
            if let Err(err) = self.save() {
                eprintln!("Error: unable to save wallet data. Details: {err}");
            }
        }
    }

    impl<R: Resolver> WalletProvider for FileOwner<R> {
        type Error = ResolverError;
        #[inline]
        fn has_utxo(&self, outpoint: Outpoint) -> bool { self.owner.has_utxo(outpoint) }
        #[inline]
        fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.owner.utxos() }
        #[inline]
        fn update_utxos(&mut self) -> Result<(), Self::Error> { self.owner.update_utxos() }
        #[inline]
        #[cfg(feature = "async")]
        async fn update_utxos_async(&mut self) -> Result<(), Self::Error> {
            self.owner.update_utxos_async().await
        }

        #[inline]
        fn register_seal(&mut self, seal: WTxoSeal) { self.owner.register_seal(seal) }
        #[inline]
        fn resolve_seals(
            &self,
            seals: impl Iterator<Item = AuthToken>,
        ) -> impl Iterator<Item = WTxoSeal> {
            self.owner.resolve_seals(seals)
        }
        #[inline]
        fn noise_seed(&self) -> Bytes32 { self.owner.noise_seed() }
        #[inline]
        fn next_address(&mut self) -> Address { self.owner.next_address() }
        #[inline]
        fn next_nonce(&mut self) -> u64 { self.owner.next_nonce() }
        #[inline]
        fn txid_resolver(&self) -> impl Fn(Txid) -> Result<WitnessStatus, Self::Error> {
            self.owner.txid_resolver()
        }
        #[inline]
        #[cfg(feature = "async")]
        fn txid_resolver_async(&self) -> impl AsyncFn(Txid) -> Result<WitnessStatus, Self::Error> {
            self.owner.txid_resolver_async()
        }

        #[inline]
        fn last_block_height(&self) -> Result<u64, Self::Error> { self.owner.last_block_height() }
        #[inline]
        #[cfg(feature = "async")]
        async fn last_block_height_async(&self) -> Result<u64, Self::Error> {
            self.owner.last_block_height_async().await
        }

        #[inline]
        fn broadcast(
            &mut self,
            tx: &Tx,
            change: Option<(Vout, u32, u32)>,
        ) -> Result<(), Self::Error> {
            self.owner.broadcast(tx, change)
        }
        #[inline]
        #[cfg(feature = "async")]
        async fn broadcast_async(
            &mut self,
            tx: &Tx,
            change: Option<(Vout, u32, u32)>,
        ) -> Result<(), Self::Error> {
            self.owner.broadcast_async(tx, change).await
        }
    }

    impl<R: Resolver> PsbtConstructor for FileOwner<R> {
        type Key = XpubDerivable;
        type Descr = RgbDescr<XpubDerivable>;

        #[inline]
        fn descriptor(&self) -> &Self::Descr { self.owner.descriptor() }
        #[inline]
        fn prev_tx(&self, txid: Txid) -> Option<UnsignedTx> { self.owner.prev_tx(txid) }
        #[inline]
        fn utxo(&self, outpoint: Outpoint) -> Option<(Utxo, ScriptPubkey)> {
            self.owner.utxo(outpoint)
        }
        #[inline]
        fn network(&self) -> Network { self.owner.network }
        #[inline]
        fn next_derivation_index(
            &mut self,
            keychain: impl Into<Keychain>,
            shift: bool,
        ) -> NormalIndex {
            self.owner.next_derivation_index(keychain, shift)
        }
    }
}

#[cfg(all(test, not(feature = "async")))]
mod test {
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
