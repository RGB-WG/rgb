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

use amplify::RawArray;
use bitcoin::hashes::Hash;
use bitcoin::ScriptBuf;
use bp::{Outpoint, Txid};

use crate::descriptor::DeriveInfo;
use crate::{RgbDescr, SpkDescriptor};

#[derive(Clone, Debug)]
pub struct RgbWallet {
    pub descr: RgbDescr,
    pub utxos: BTreeSet<Utxo>,
}

impl RgbWallet {
    pub fn new(descr: RgbDescr) -> Self {
        Self {
            descr,
            utxos: empty!(),
        }
    }

    pub fn update(&mut self, resolver: &mut impl Resolver) -> Result<(), String> {
        const STEP: u32 = 20;
        for app in [0, 1, 9, 10] {
            let mut index = 0;
            loop {
                debug!("Requesting {STEP} scripts from the Electrum server");
                let scripts = self.descr.derive(app, index..(index + STEP));
                let set = resolver.resolve_utxo(scripts)?;
                if set.is_empty() {
                    break;
                }
                debug!("Electrum server returned {} UTXOs", set.len());
                self.utxos.extend(set);
                index += STEP;
            }
        }

        Ok(())
    }

    pub fn utxo(&self, outpoint: Outpoint) -> Option<&Utxo> {
        self.utxos.iter().find(|utxo| utxo.outpoint == outpoint)
    }
}
