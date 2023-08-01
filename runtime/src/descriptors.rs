// RGB smart contract wallet runtime
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

use bp::{Derive, DeriveSet, DeriveXOnly, NormalIndex, ScriptPubkey, XpubDescriptor};
use dbc::tapret::TapretCommitment;

#[derive(Clone, Eq, PartialEq, Hash, Debug, From)]
pub struct TapretKey<K: DeriveXOnly = XpubDescriptor> {
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

impl<K: DeriveXOnly> Derive<ScriptPubkey> for TapretKey<K> {
    fn derive(
        &self,
        change: impl Into<NormalIndex>,
        index: impl Into<NormalIndex>,
    ) -> ScriptPubkey {
        // TODO: Apply tweaks
        let internal_key = self.internal_key.derive(change, index);
        ScriptPubkey::p2tr_key_only(internal_key)
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, From)]
pub enum DescriptorRgb<S: DeriveSet = XpubDescriptor> {
    #[from]
    TapretKey(TapretKey<S::XOnly>),
}

impl<S: DeriveSet> Derive<ScriptPubkey> for DescriptorRgb<S> {
    fn derive(
        &self,
        change: impl Into<NormalIndex>,
        index: impl Into<NormalIndex>,
    ) -> ScriptPubkey {
        match self {
            DescriptorRgb::TapretKey(d) => d.derive(change, index),
        }
    }
}
