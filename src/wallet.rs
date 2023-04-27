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

use bitcoin::ScriptBuf;
use bp::{Chain, Outpoint};

use crate::descriptor::DeriveInfo;
use crate::{RgbDescr, SpkDescriptor};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Display)]
pub enum MiningStatus {
    #[display("~")]
    Mempool,
    #[display(inner)]
    Blockchain(u32),
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct Utxo {
    pub outpoint: Outpoint,
    pub status: MiningStatus,
    pub amount: u64,
    pub derivation: DeriveInfo,
}

pub trait Resolver {
    fn resolve_utxo<'s>(
        &mut self,
        scripts: BTreeMap<DeriveInfo, ScriptBuf>,
    ) -> Result<BTreeSet<Utxo>, String>;
}

#[derive(Clone, Debug)]
pub struct RgbWallet {
    pub descr: RgbDescr,
    pub utxos: BTreeSet<Utxo>,
}

impl RgbWallet {
    pub fn with(descr: RgbDescr, resolver: &mut impl Resolver) -> Result<Self, String> {
        let mut utxos = BTreeSet::new();

        const STEP: u32 = 20;
        for app in [0, 1, 10, 20, 30, 40, 50, 60] {
            let mut index = 0;
            loop {
                debug!("Requesting {STEP} scripts from the Electrum server");
                let scripts = descr.derive(app, index..(index + STEP));
                let set = resolver.resolve_utxo(scripts)?;
                if set.is_empty() {
                    break;
                }
                debug!("Electrum server returned {} UTXOs", set.len());
                utxos.extend(set);
                index += STEP;
            }
        }

        Ok(Self { descr, utxos })
    }

    pub fn utxo(&self, outpoint: Outpoint) -> Option<&Utxo> {
        self.utxos.iter().find(|utxo| utxo.outpoint == outpoint)
    }
}

pub trait DefaultResolver {
    fn default_resolver(&self) -> String;
}

impl DefaultResolver for Chain {
    fn default_resolver(&self) -> String {
        match self {
            Chain::Bitcoin => s!("blockstream.info:110"),
            Chain::Testnet3 => s!("blockstream.info:143"),
            Chain::Regtest => s!("localhost:50001"),
            chain => {
                panic!("no default server is known for {chain}, please provide a custom URL")
            }
        }
    }
}
