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

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt::{self, Display, Formatter};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use amplify::{Wrapper, WrapperMut};
use bpstd::dbc::opret::OpretProof;
use bpstd::dbc::tapret::{TapretCommitment, TapretProof};
use bpstd::seals::TxoSeal;
use bpstd::{
    dbc, Derive, DeriveSet, DeriveXOnly, DerivedScript, Descriptor, KeyOrigin, Keychain,
    LegacyKeySig, LegacyPk, NormalIndex, SigScript, SpkClass, StdDescr, TapDerivation, TapScript,
    TapTree, TaprootKeySig, Terminal, Tr, TrKey, Witness, XOnlyPk, XpubAccount, XpubDerivable,
};
use commit_verify::CommitVerify;
use indexmap::IndexMap;

pub trait DescriptorRgb<D: dbc::Proof, K = XpubDerivable, V = ()>: Descriptor<K, V> {
    fn add_seal(&self, seal: TxoSeal<D>);
}

#[derive(Clone, Eq, PartialEq, Debug, From)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(transparent))]
pub struct SealDescr<D: dbc::Proof>(BTreeSet<TxoSeal<D>>);

impl<D: dbc::Proof> Deref for SealDescr<D> {
    type Target = BTreeSet<TxoSeal<D>>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<D: dbc::Proof> DerefMut for SealDescr<D> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<D: dbc::Proof> Wrapper for SealDescr<D> {
    type Inner = BTreeSet<TxoSeal<D>>;
    fn from_inner(inner: Self::Inner) -> Self { Self(inner) }
    fn as_inner(&self) -> &Self::Inner { &self.0 }
    fn into_inner(self) -> Self::Inner { self.0 }
}

impl<D: dbc::Proof> WrapperMut for SealDescr<D> {
    fn as_inner_mut(&mut self) -> &mut Self::Inner { &mut self.0 }
}

impl<D: dbc::Proof> Default for SealDescr<D> {
    fn default() -> Self { Self(empty!()) }
}

impl<D: dbc::Proof> Display for SealDescr<D> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("seals(")?;
        let mut iter = self.0.iter().peekable();
        while let Some(seal) = iter.next() {
            Display::fmt(seal, f)?;
            if iter.peek().is_some() {
                f.write_str(",")?;
            }
        }
        f.write_str(")")
    }
}

#[derive(Clone, Display)]
#[display("opret({descr}, {seals})")]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(
        rename_all = "camelCase",
        bound(
            serialize = "K::Compr: serde::Serialize, K::XOnly: serde::Serialize",
            deserialize = "K::Compr: serde::Deserialize<'de>, K::XOnly: serde::Deserialize<'de>"
        )
    )
)]
pub struct Opret<K: DeriveSet = XpubDerivable> {
    pub descr: StdDescr<K>,
    pub seals: SealDescr<OpretProof>,
}

impl<K: DeriveSet> Opret<K> {
    pub fn new_unfunded(descr: impl Into<StdDescr<K>>) -> Self {
        Self {
            descr: descr.into(),
            seals: empty!(),
        }
    }
}

impl<K: DeriveSet> Derive<DerivedScript> for Opret<K> {
    fn default_keychain(&self) -> Keychain { self.descr.default_keychain() }
    fn keychains(&self) -> BTreeSet<Keychain> { self.descr.keychains() }
    fn derive(
        &self,
        keychain: impl Into<Keychain>,
        index: impl Into<NormalIndex>,
    ) -> impl Iterator<Item = DerivedScript> {
        self.descr.derive(keychain, index)
    }
}

impl<K: DeriveSet> Descriptor<K> for Opret<K>
where
    Self: Clone,
    StdDescr<K>: Descriptor<K>,
{
    fn class(&self) -> SpkClass { self.descr.class() }
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where K: 'a {
        self.descr.keys()
    }
    fn vars<'a>(&'a self) -> impl Iterator<Item = &'a ()>
    where (): 'a {
        self.descr.vars()
    }
    fn xpubs(&self) -> impl Iterator<Item = &XpubAccount> { self.descr.xpubs() }
    fn legacy_keyset(&self, terminal: Terminal) -> IndexMap<LegacyPk, KeyOrigin> {
        self.descr.legacy_keyset(terminal)
    }
    fn xonly_keyset(&self, terminal: Terminal) -> IndexMap<XOnlyPk, TapDerivation> {
        self.descr.xonly_keyset(terminal)
    }
    fn legacy_witness(
        &self,
        keysigs: HashMap<&KeyOrigin, LegacyKeySig>,
    ) -> Option<(SigScript, Witness)> {
        self.descr.legacy_witness(keysigs)
    }
    fn taproot_witness(&self, keysigs: HashMap<&KeyOrigin, TaprootKeySig>) -> Option<Witness> {
        self.descr.taproot_witness(keysigs)
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "camelCase"))]
pub struct Tapret<K: DeriveXOnly = XpubDerivable> {
    pub tr: Tr<K>,
    pub tweaks: BTreeMap<Terminal, BTreeSet<TapretCommitment>>,
    pub seals: SealDescr<TapretProof>,
}

impl<K: DeriveXOnly> Display for Tapret<K> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("tapret(")?;
        match &self.tr {
            Tr::KeyOnly(d) => Display::fmt(d.as_internal_key(), f)?,
        }
        if !self.tweaks.is_empty() {
            f.write_str(",tweaks(")?;
            let mut iter1 = self.tweaks.iter().peekable();
            while let Some((term, tweaks)) = iter1.next() {
                write!(f, "{term}=")?;
                let mut iter2 = tweaks.iter().peekable();
                while let Some(tweak) = iter2.next() {
                    write!(f, "{tweak}")?;
                    if iter2.peek().is_some() {
                        f.write_str(",")?;
                    }
                }
                if iter1.peek().is_some() {
                    f.write_str(";")?;
                }
            }
            f.write_str(")")?;
        }
        if !self.seals.is_empty() {
            write!(f, ",{}", self.seals)?;
        }
        f.write_str(")")
    }
}

impl<K: DeriveXOnly> Tapret<K> {
    pub fn key_only_unfunded(internal_key: K) -> Self {
        Self {
            tr: Tr::KeyOnly(TrKey::from(internal_key)),
            tweaks: empty!(),
            seals: empty!(),
        }
    }

    pub fn add_tweak(&mut self, terminal: Terminal, tweak: TapretCommitment) {
        self.tweaks.entry(terminal).or_default().insert(tweak);
    }
}

impl<K: DeriveXOnly> Derive<DerivedScript> for Tapret<K> {
    fn default_keychain(&self) -> Keychain { self.tr.default_keychain() }
    fn keychains(&self) -> BTreeSet<Keychain> { self.tr.keychains() }
    fn derive(
        &self,
        keychain: impl Into<Keychain>,
        index: impl Into<NormalIndex>,
    ) -> impl Iterator<Item = DerivedScript> {
        let keychain = keychain.into();
        let index = index.into();
        let terminal = Terminal::new(keychain, index);
        self.tr
            .as_internal_key()
            .derive(keychain, index)
            .flat_map(move |internal_key| {
                self.tweaks
                    .get(&terminal)
                    .into_iter()
                    .flatten()
                    .map(move |tweak| {
                        let script_commitment = TapScript::commit(tweak);
                        let tap_tree = TapTree::with_single_leaf(script_commitment);
                        DerivedScript::TaprootScript(internal_key.into(), tap_tree)
                    })
            })
    }
}

impl<K: DeriveXOnly> Descriptor<K> for Tapret<K> {
    fn class(&self) -> SpkClass { self.tr.class() }
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where K: 'a {
        self.tr.keys()
    }
    fn vars<'a>(&'a self) -> impl Iterator<Item = &'a ()>
    where (): 'a {
        self.tr.vars()
    }
    fn xpubs(&self) -> impl Iterator<Item = &XpubAccount> { self.tr.xpubs() }
    fn legacy_keyset(&self, terminal: Terminal) -> IndexMap<LegacyPk, KeyOrigin> {
        self.tr.legacy_keyset(terminal)
    }
    fn xonly_keyset(&self, terminal: Terminal) -> IndexMap<XOnlyPk, TapDerivation> {
        self.tr.xonly_keyset(terminal)
    }
    fn legacy_witness(
        &self,
        keysigs: HashMap<&KeyOrigin, LegacyKeySig>,
    ) -> Option<(SigScript, Witness)> {
        self.tr.legacy_witness(keysigs)
    }
    fn taproot_witness(&self, keysigs: HashMap<&KeyOrigin, TaprootKeySig>) -> Option<Witness> {
        self.tr.taproot_witness(keysigs)
    }
}
