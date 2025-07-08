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

use amplify::{Bytes32, Wrapper, WrapperMut};
use bpstd::dbc::tapret::TapretCommitment;
use bpstd::seals::WTxoSeal;
use bpstd::{
    ControlBlock, Derive, DeriveCompr, DeriveKey, DeriveLegacy, DeriveSet, DeriveXOnly,
    DerivedScript, Descriptor, KeyOrigin, Keychain, LegacyKeySig, LegacyPk, NormalIndex,
    RedeemScript, SigScript, SpkClass, StdDescr, TapDerivation, TapScript, TapTree, TaprootKeySig,
    Terminal, Tr, TrKey, Witness, WitnessScript, XOnlyPk, XpubAccount, XpubDerivable,
};
use commit_verify::CommitVerify;
use indexmap::IndexMap;

pub trait DescriptorRgb<K = XpubDerivable, V = ()>: Descriptor<K, V> {
    fn add_seal(&self, seal: WTxoSeal);
}

#[derive(Wrapper, WrapperMut, Clone, Eq, PartialEq, Debug, Default, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(transparent))]
pub struct SealDescr(BTreeSet<WTxoSeal>);

impl Display for SealDescr {
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

#[derive(Wrapper, WrapperMut, Clone, PartialEq, Eq, Debug, Default, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(transparent))]
pub struct TapretWeaks(BTreeMap<Terminal, BTreeSet<TapretCommitment>>);

impl Display for TapretWeaks {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("tweaks(")?;
        let mut iter1 = self.iter().peekable();
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
        f.write_str(")")
    }
}

#[derive(Clone, Display, From)]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(
        rename_all = "camelCase",
        bound(
            serialize = "K::Legacy: serde::Serialize, K::Compr: serde::Serialize, K::XOnly: \
                         serde::Serialize",
            deserialize = "K::Legacy: serde::Deserialize<'de>, K::Compr: serde::Deserialize<'de>, \
                           K::XOnly: serde::Deserialize<'de>"
        )
    )
)]
// TODO: Remove DeriveSet bound
enum RgbDeriver<K: DeriveSet = XpubDerivable> {
    #[from]
    #[display(inner)]
    OpretOnly(StdDescr<K>),

    #[display("{tr},{tweaks}")]
    Universal {
        tr: Tr<K::XOnly>,
        tweaks: TapretWeaks,
    },
}

impl<K: DeriveSet> Derive<DerivedScript> for RgbDeriver<K> {
    fn default_keychain(&self) -> Keychain {
        match self {
            RgbDeriver::OpretOnly(d) => d.default_keychain(),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.default_keychain(),
        }
    }

    fn keychains(&self) -> BTreeSet<Keychain> {
        match self {
            RgbDeriver::OpretOnly(d) => d.keychains(),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.keychains(),
        }
    }

    fn derive(
        &self,
        keychain: impl Into<Keychain>,
        index: impl Into<NormalIndex>,
    ) -> impl Iterator<Item = DerivedScript> {
        match self {
            RgbDeriver::OpretOnly(d) => d.derive(keychain, index).collect::<Vec<_>>().into_iter(),
            RgbDeriver::Universal { tr, tweaks } => {
                let keychain = keychain.into();
                let index = index.into();
                let terminal = Terminal::new(keychain, index);
                let mut vec = Vec::with_capacity(tweaks.0.len());
                for internal_key in tr.as_internal_key().derive(keychain, index) {
                    vec.push(DerivedScript::TaprootKeyOnly(internal_key.into()));
                    for tweak in tweaks.get(&terminal).into_iter().flatten() {
                        let script_commitment = TapScript::commit(tweak);
                        let tap_tree = TapTree::with_single_leaf(script_commitment);
                        let script = DerivedScript::TaprootScript(internal_key.into(), tap_tree);
                        vec.push(script);
                    }
                }
                vec.into_iter()
            }
        }
    }
}

impl<K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly>
    Descriptor<K> for RgbDeriver<K>
{
    fn class(&self) -> SpkClass {
        match self {
            RgbDeriver::OpretOnly(d) => d.class(),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.class(),
        }
    }
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where K: 'a {
        match self {
            RgbDeriver::OpretOnly(d) => d.keys().collect::<Vec<_>>().into_iter(),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.keys().collect::<Vec<_>>().into_iter(),
        }
    }
    fn vars<'a>(&'a self) -> impl Iterator<Item = &'a ()>
    where (): 'a {
        match self {
            RgbDeriver::OpretOnly(d) => d.vars().collect::<Vec<_>>().into_iter(),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.vars().collect::<Vec<_>>().into_iter(),
        }
    }
    fn xpubs(&self) -> impl Iterator<Item = &XpubAccount> {
        match self {
            RgbDeriver::OpretOnly(d) => d.xpubs().collect::<Vec<_>>().into_iter(),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.xpubs().collect::<Vec<_>>().into_iter(),
        }
    }
    fn legacy_keyset(&self, terminal: Terminal) -> IndexMap<LegacyPk, KeyOrigin> {
        match self {
            RgbDeriver::OpretOnly(d) => d.legacy_keyset(terminal),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.legacy_keyset(terminal),
        }
    }
    fn xonly_keyset(&self, terminal: Terminal) -> IndexMap<XOnlyPk, TapDerivation> {
        match self {
            RgbDeriver::OpretOnly(d) => d.xonly_keyset(terminal),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.xonly_keyset(terminal),
        }
    }
    fn legacy_witness(
        &self,
        keysigs: IndexMap<&KeyOrigin, LegacyKeySig>,
        redeem_script: Option<RedeemScript>,
        witness_script: Option<WitnessScript>,
    ) -> Option<(SigScript, Option<Witness>)> {
        match self {
            RgbDeriver::OpretOnly(d) => d.legacy_witness(keysigs, redeem_script, witness_script),
            RgbDeriver::Universal { tr, tweaks: _ } => {
                tr.legacy_witness(keysigs, redeem_script, witness_script)
            }
        }
    }
    fn taproot_witness(
        &self,
        cb: Option<&ControlBlock>,
        keysigs: IndexMap<&KeyOrigin, TaprootKeySig>,
    ) -> Option<Witness> {
        match self {
            RgbDeriver::OpretOnly(d) => d.taproot_witness(cb, keysigs),
            RgbDeriver::Universal { tr, tweaks: _ } => tr.taproot_witness(cb, keysigs),
        }
    }
}

#[derive(Clone, Display)]
#[display("rgb({deriver},{seals},noise({noise:x}))")]
#[cfg_attr(
    feature = "serde",
    derive(Serialize, Deserialize),
    serde(
        rename_all = "camelCase",
        bound(
            serialize = "K::Legacy: serde::Serialize, K::Compr: serde::Serialize, K::XOnly: \
                         serde::Serialize",
            deserialize = "K::Legacy: serde::Deserialize<'de>, K::Compr: serde::Deserialize<'de>, \
                           K::XOnly: serde::Deserialize<'de>"
        )
    )
)]
pub struct RgbDescr<K: DeriveSet = XpubDerivable> {
    deriver: RgbDeriver<K>,
    seals: SealDescr,
    noise: Bytes32,
    nonce: u64,
}

impl<K: DeriveSet> RgbDescr<K> {
    pub fn new_unfunded(deriver: impl Into<StdDescr<K>>, noise: impl Into<[u8; 32]>) -> Self {
        let deriver = match deriver.into() {
            StdDescr::Wpkh(d) => RgbDeriver::OpretOnly(StdDescr::Wpkh(d)),
            StdDescr::TrKey(tr) => RgbDeriver::Universal { tr: Tr::KeyOnly(tr), tweaks: empty!() },
            _ => unreachable!(),
        };
        Self {
            deriver,
            seals: empty!(),
            noise: noise.into().into(),
            nonce: 0,
        }
    }

    pub fn key_only_unfunded(internal_key: K, noise: impl Into<[u8; 32]>) -> Self
    where K: DeriveSet<XOnly = K> + DeriveKey<XOnlyPk> {
        Self {
            deriver: RgbDeriver::Universal {
                tr: Tr::KeyOnly(TrKey::from(internal_key)),
                tweaks: empty!(),
            },
            seals: empty!(),
            noise: noise.into().into(),
            nonce: 0,
        }
    }

    pub fn next_nonce(&mut self) -> u64 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }

    pub fn noise(&self) -> Bytes32 { self.noise }

    pub fn seals(&self) -> impl Iterator<Item = WTxoSeal> + use<'_, K> {
        self.seals.iter().copied()
    }

    pub fn add_seal(&mut self, seal: WTxoSeal) { self.seals.insert(seal); }

    pub fn add_tweak(&mut self, terminal: Terminal, tweak: TapretCommitment) {
        match &mut self.deriver {
            RgbDeriver::OpretOnly(_) => {
                panic!("attempting to add tapret tweaks to an opret-only wallet")
            }
            RgbDeriver::Universal { tr: _, tweaks } => {
                tweaks.entry(terminal).or_default().insert(tweak);
            }
        }
    }
}

impl<K: DeriveSet> Derive<DerivedScript> for RgbDescr<K> {
    fn default_keychain(&self) -> Keychain { self.deriver.default_keychain() }
    fn keychains(&self) -> BTreeSet<Keychain> { self.deriver.keychains() }
    fn derive(
        &self,
        keychain: impl Into<Keychain>,
        index: impl Into<NormalIndex>,
    ) -> impl Iterator<Item = DerivedScript> {
        self.deriver.derive(keychain, index)
    }
}

impl<K: DeriveSet<Legacy = K, Compr = K, XOnly = K> + DeriveLegacy + DeriveCompr + DeriveXOnly>
    Descriptor<K> for RgbDescr<K>
{
    fn class(&self) -> SpkClass { self.deriver.class() }
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where K: 'a {
        self.deriver.keys()
    }
    fn vars<'a>(&'a self) -> impl Iterator<Item = &'a ()>
    where (): 'a {
        self.deriver.vars()
    }
    fn xpubs(&self) -> impl Iterator<Item = &XpubAccount> { self.deriver.xpubs() }
    fn legacy_keyset(&self, terminal: Terminal) -> IndexMap<LegacyPk, KeyOrigin> {
        self.deriver.legacy_keyset(terminal)
    }
    fn xonly_keyset(&self, terminal: Terminal) -> IndexMap<XOnlyPk, TapDerivation> {
        self.deriver.xonly_keyset(terminal)
    }
    fn legacy_witness(
        &self,
        keysigs: IndexMap<&KeyOrigin, LegacyKeySig>,
        redeem_script: Option<RedeemScript>,
        witness_script: Option<WitnessScript>,
    ) -> Option<(SigScript, Option<Witness>)> {
        self.deriver
            .legacy_witness(keysigs, redeem_script, witness_script)
    }
    fn taproot_witness(
        &self,
        cb: Option<&ControlBlock>,
        keysigs: IndexMap<&KeyOrigin, TaprootKeySig>,
    ) -> Option<Witness> {
        self.deriver.taproot_witness(cb, keysigs)
    }
}

#[cfg(test)]
mod test {
    use core::str::FromStr;

    use bpstd::Wpkh;

    use super::*;

    #[test]
    fn descr_serde() {
        let s = "[643a7adc/86'/1'/0']tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1;9;10>/*";
        let xpub = XpubDerivable::from_str(s).unwrap();
        let descr = RgbDescr::<XpubDerivable>::new_unfunded(Wpkh::from(xpub), [0u8; 32]);

        let toml = toml::to_string(&descr).unwrap();
        let descr2: RgbDescr<XpubDerivable> = toml::from_str(&toml).unwrap();

        assert_eq!(descr.to_string(), descr2.to_string());
        assert_eq!(
            toml,
            r#"seals = []
noise = "0000000000000000000000000000000000000000000000000000000000000000"
nonce = 0

[deriver.opretOnly]
wpkh = "[643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1;9;10>/*"
"#
        );
    }
}
