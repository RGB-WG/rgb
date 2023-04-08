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

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub enum MiningStatus {
    Mempool,
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

#[cfg(feature = "electrum")]
mod _electrum {
    use bitcoin::ScriptBuf;
    use bp::Chain;
    use electrum_client::{ElectrumApi, ListUnspentRes};

    use super::*;

    impl DefaultResolver for bp::Chain {
        fn default_resolver(&self) -> String {
            match self {
                Chain::Bitcoin => s!("blockstream.info:110"),
                Chain::Testnet3 => s!("blockstream.info:143"),
                chain => {
                    panic!("no default server is known for {chain}, please provide a custom URL")
                }
            }
        }
    }

    impl Resolver for electrum_client::Client {
        fn resolve_utxo<'s>(
            &mut self,
            scripts: BTreeMap<DeriveInfo, ScriptBuf>,
        ) -> Result<BTreeSet<Utxo>, String> {
            Ok(self
                .batch_script_list_unspent(scripts.values().map(ScriptBuf::as_script))
                .map_err(|err| err.to_string())?
                .into_iter()
                .zip(scripts.into_keys())
                .flat_map(|(list, derivation)| {
                    list.into_iter()
                        .map(move |res| Utxo::with(derivation.clone(), res))
                })
                .collect())
        }
    }

    impl Utxo {
        fn with(derivation: DeriveInfo, res: ListUnspentRes) -> Self {
            Utxo {
                status: if res.height == 0 {
                    MiningStatus::Mempool
                } else {
                    MiningStatus::Blockchain(res.height as u32)
                },
                outpoint: Outpoint::new(
                    Txid::from_raw_array(res.tx_hash.to_byte_array()),
                    res.tx_pos as u32,
                ),
                derivation,
                amount: res.value,
            }
        }
    }
}
