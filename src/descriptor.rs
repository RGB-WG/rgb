// RGB smart contracts for Bitcoin & Lightning
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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};
use std::ops::Range;

use bitcoin::bip32::{ChildNumber, ExtendedPubKey};
use bitcoin::secp256k1::SECP256K1;
use bitcoin::ScriptBuf;
use bp::dbc::tapret::{TapretCommitment, TapretPathProof, TapretProof};
use bp::{ScriptPubkey, TapNodeHash};
use commit_verify::ConvolveCommit;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Display)]
#[display("*/{app}/{index}")]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalPath {
    pub app: u32,
    pub index: u32,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub struct DeriveInfo {
    pub terminal: TerminalPath,
    pub tweak: Option<TapretCommitment>,
}

impl Display for DeriveInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.terminal, f)?;
        if let Some(tweak) = &self.tweak {
            write!(f, "%{tweak}")?;
        }
        Ok(())
    }
}

impl DeriveInfo {
    pub fn with(app: u32, index: u32, tweak: Option<TapretCommitment>) -> Self {
        DeriveInfo {
            terminal: TerminalPath { app, index },
            tweak,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tapret {
    pub xpub: ExtendedPubKey,
    // pub script: Option<>,
    pub taprets: BTreeMap<TerminalPath, BTreeSet<TapretCommitment>>,
}

impl Display for Tapret {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("tapret(")?;
        Display::fmt(&self.xpub, f)?;
        let mut first = true;
        if f.alternate() {
            for (terminal, taprets) in &self.taprets {
                if first {
                    f.write_str(",")?;
                    first = false;
                } else {
                    f.write_str(" ")?;
                }
                Display::fmt(terminal, f)?;
                for tapret in taprets {
                    f.write_str("&")?;
                    Display::fmt(tapret, f)?;
                }
            }
        } else {
            f.write_str(", ...")?;
        }
        f.write_str(")")
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Display)]
#[display(inner)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RgbDescr {
    Tapret(Tapret),
}

pub trait SpkDescriptor {
    // TODO: Replace index with UnhardenedIndex
    fn derive(&self, app: u32, indexes: Range<u32>) -> BTreeMap<DeriveInfo, ScriptBuf>;
}

impl SpkDescriptor for Tapret {
    fn derive(&self, app: u32, indexes: Range<u32>) -> BTreeMap<DeriveInfo, ScriptBuf> {
        let mut spks = BTreeMap::new();

        for index in indexes {
            let key = self
                .xpub
                .derive_pub(SECP256K1, &[
                    ChildNumber::from_normal_idx(app)
                        .expect("application index must be unhardened"),
                    ChildNumber::from_normal_idx(index)
                        .expect("derivation index must be unhardened"),
                ])
                .expect("unhardened derivation");

            let xonly = key.to_x_only_pub();
            spks.insert(
                DeriveInfo::with(app, index, None),
                ScriptBuf::new_v1_p2tr(SECP256K1, xonly, None),
            );
            for tweak in self
                .taprets
                .get(&TerminalPath { app, index })
                .into_iter()
                .flatten()
            {
                let script = ScriptPubkey::p2tr(xonly.into(), None::<TapNodeHash>);
                let proof = TapretProof {
                    path_proof: TapretPathProof::root(tweak.nonce),
                    internal_pk: xonly.into(),
                };
                let (spk, _) = script
                    .convolve_commit(&proof, &tweak.mpc)
                    .expect("malicious tapret value - an inverse of a key");
                spks.insert(
                    DeriveInfo::with(app, index, Some(tweak.clone())),
                    ScriptBuf::from_bytes(spk.to_inner()),
                );
            }
        }
        spks
    }
}

impl SpkDescriptor for RgbDescr {
    fn derive(&self, app: u32, indexes: Range<u32>) -> BTreeMap<DeriveInfo, ScriptBuf> {
        match self {
            RgbDescr::Tapret(tapret) => tapret.derive(app, indexes),
        }
    }
}
