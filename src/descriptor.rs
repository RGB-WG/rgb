// RGB standard library for working with smart contracts on Bitcoin & Lightning
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

use std::collections::BTreeMap;
use std::ops::Range;
use std::str::FromStr;
use std::{iter, vec};

use bpstd::{
    CompressedPk, Derive, DeriveCompr, DeriveSet, DeriveXOnly, DerivedScript, Idx, IndexError,
    IndexParseError, KeyOrigin, NormalIndex, TapDerivation, Terminal, XOnlyPk, XpubDerivable,
    XpubSpec,
};
use dbc::tapret::TapretCommitment;
use descriptors::{Descriptor, DescriptorStd, TrKey};
use indexmap::IndexMap;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Display)]
#[derive(Serialize, Deserialize)]
#[serde(crate = "serde_crate", rename_all = "camelCase")]
#[repr(u8)]
pub enum RgbKeychain {
    #[display("0", alt = "0")]
    External = 0,

    #[display("1", alt = "1")]
    Internal = 1,

    #[display("9", alt = "9")]
    Rgb = 9,

    #[display("10", alt = "10")]
    Tapret = 10,
}

impl RgbKeychain {
    pub fn is_seal(self) -> bool { self == Self::Rgb || self == Self::Tapret }
}

impl FromStr for RgbKeychain {
    type Err = IndexParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match NormalIndex::from_str(s)? {
            NormalIndex::ZERO => Ok(RgbKeychain::External),
            NormalIndex::ONE => Ok(RgbKeychain::Internal),
            val => Err(IndexError {
                what: "non-standard keychain",
                invalid: val.index(),
                start: 0,
                end: 1,
            }
            .into()),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(crate = "serde_crate", rename_all = "camelCase")]
pub struct TapretKey<K: DeriveXOnly = XpubDerivable> {
    pub internal_key: K,
    pub tweaks: BTreeMap<NormalIndex, TapretCommitment>,
}

impl<K: DeriveXOnly> TapretKey<K> {
    pub fn new_unfunded(internal_key: K) -> Self {
        TapretKey {
            internal_key,
            tweaks: empty!(),
        }
    }
}

impl<K: DeriveXOnly> Derive<DerivedScript> for TapretKey<K> {
    fn keychains(&self) -> Range<u8> {
        0..2 /* FIXME */
    }

    fn derive(&self, change: u8, index: impl Into<NormalIndex>) -> DerivedScript {
        // TODO: Apply tweaks
        let internal_key = self.internal_key.derive(change, index);
        DerivedScript::TaprootKeyOnly(internal_key.into())
    }
}

impl<K: DeriveXOnly> From<K> for TapretKey<K> {
    fn from(tr: K) -> Self {
        TapretKey {
            internal_key: tr,
            tweaks: none!(),
        }
    }
}

impl<K: DeriveXOnly> From<TrKey<K>> for TapretKey<K> {
    fn from(tr: TrKey<K>) -> Self {
        TapretKey {
            internal_key: tr.into_internal_key(),
            tweaks: none!(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default, From)]
#[derive(Serialize, Deserialize)]
#[serde(
    crate = "serde_crate",
    rename_all = "camelCase",
    bound(
        serialize = "S::XOnly: serde::Serialize",
        deserialize = "S::XOnly: serde::Deserialize<'de>"
    )
)]
pub enum DescriptorRgb<S: DeriveSet = XpubDerivable> {
    #[default]
    None,

    #[from]
    TapretKey(TapretKey<S::XOnly>),
}

impl<S: DeriveSet> Derive<DerivedScript> for DescriptorRgb<S> {
    fn keychains(&self) -> Range<u8> {
        match self {
            DescriptorRgb::None => Range::default(),
            DescriptorRgb::TapretKey(d) => d.keychains(),
        }
    }

    fn derive(&self, change: u8, index: impl Into<NormalIndex>) -> DerivedScript {
        match self {
            DescriptorRgb::None => todo!(),
            DescriptorRgb::TapretKey(d) => d.derive(change, index),
        }
    }
}

impl<K: DeriveSet<Compr = K, XOnly = K> + DeriveCompr + DeriveXOnly> Descriptor<K>
    for DescriptorRgb<K>
where Self: Derive<DerivedScript>
{
    type KeyIter<'k> = vec::IntoIter<&'k K> where Self: 'k, K: 'k;
    type VarIter<'v> = iter::Empty<&'v ()> where Self: 'v, (): 'v;
    type XpubIter<'x> = vec::IntoIter<&'x XpubSpec> where Self: 'x;

    fn keys(&self) -> Self::KeyIter<'_> { todo!() }

    fn vars(&self) -> Self::VarIter<'_> { todo!() }

    fn xpubs(&self) -> Self::XpubIter<'_> { todo!() }

    fn compr_keyset(&self, _terminal: Terminal) -> IndexMap<CompressedPk, KeyOrigin> { todo!() }

    fn xonly_keyset(&self, _terminal: Terminal) -> IndexMap<XOnlyPk, TapDerivation> { todo!() }
}

impl From<DescriptorStd> for DescriptorRgb {
    fn from(descr: DescriptorStd) -> Self {
        match descr {
            DescriptorStd::Wpkh(_) => todo!(),
            DescriptorStd::TrKey(tr) => DescriptorRgb::TapretKey(tr.into()),
            _ => todo!(),
        }
    }
}
