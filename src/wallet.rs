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

#[cfg(feature = "electrum")]
#[derive(Wrapper, WrapperMut, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
pub struct BlockchainResolver(electrum_client::Client);

impl BlockchainResolver {
    #[cfg(feature = "electrum")]
    pub fn with(url: &str) -> Result<Self, electrum_client::Error> {
        electrum_client::Client::new(url).map(Self)
    }
}

#[cfg(feature = "electrum")]
mod _electrum {
    use std::convert::Infallible;

    use bitcoin::ScriptBuf;
    use bp::{Chain, Tx, TxIn, TxOut, VarIntArray};
    use electrum_client::{ElectrumApi, Error, ListUnspentRes};
    use rgbstd::resolvers::ResolveHeight;
    use rgbstd::validation::{ResolveTx, TxResolverError};

    use super::*;

    impl DefaultResolver for Chain {
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

    impl Resolver for BlockchainResolver {
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

    impl ResolveTx for BlockchainResolver {
        fn resolve_tx(&self, txid: Txid) -> Result<Tx, TxResolverError> {
            let tx = self
                .0
                .transaction_get(&bitcoin::Txid::from_byte_array(txid.to_raw_array()))
                .map_err(|err| match err {
                    Error::Message(_) | Error::Protocol(_) => TxResolverError::Unknown(txid),
                    err => TxResolverError::Other(txid, err.to_string()),
                })?;
            Ok(Tx {
                version: (tx.version as u8)
                    .try_into()
                    .expect("non-consensus tx version"),
                inputs: VarIntArray::try_from_iter(tx.input.into_iter().map(|txin| TxIn {
                    prev_output: Outpoint::new(
                        txin.previous_output.txid.to_byte_array().into(),
                        txin.previous_output.vout,
                    ),
                    sig_script: txin.script_sig.to_bytes().into(),
                    sequence: txin.sequence.0.into(),
                }))
                .expect("consensus-invalid transaction"),
                outputs: VarIntArray::try_from_iter(tx.output.into_iter().map(|txout| TxOut {
                    value: txout.value.into(),
                    script_pubkey: txout.script_pubkey.to_bytes().into(),
                }))
                .expect("consensus-invalid transaction"),
                lock_time: tx.lock_time.to_consensus_u32().into(),
            })
        }
    }

    impl ResolveHeight for BlockchainResolver {
        type Error = Infallible;
        fn resolve_height(&mut self, _txid: Txid) -> Result<u32, Self::Error> {
            // TODO: find a way how to resolve transaction height
            Ok(0)
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
