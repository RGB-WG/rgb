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
use bp::Outpoint;

use crate::descriptor::DeriveInfo;
use crate::{RgbDescr, SpkDescriptor};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Display)]
#[derive(Serialize, Deserialize)]
pub enum MiningStatus {
    #[display("~")]
    Mempool,
    #[display(inner)]
    Blockchain(u32),
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub struct Utxo {
    pub outpoint: Outpoint,
    pub status: MiningStatus,
    pub amount: u64,
    pub derivation: DeriveInfo,
}

pub trait Resolver {
    fn resolve_utxo(
        &mut self,
        scripts: BTreeMap<DeriveInfo, ScriptBuf>,
    ) -> Result<BTreeSet<Utxo>, String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
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
                #[cfg(feature = "log")]
                debug!("Requesting {STEP} scripts from the Electrum server");
                let scripts = self.descr.derive(app, index..(index + STEP));
                let set = resolver.resolve_utxo(scripts)?;
                if set.is_empty() {
                    break;
                }
                #[cfg(feature = "log")]
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

pub trait DefaultResolver {
    fn default_resolver(&self) -> String;
}

#[cfg(feature = "electrum")]
#[derive(Wrapper, WrapperMut, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
pub struct BlockchainResolver(electrum_client::Client);

#[cfg(feature = "electrum")]
impl BlockchainResolver {
    pub fn with(url: &str) -> Result<Self, electrum_client::Error> {
        electrum_client::Client::new(url).map(Self)
    }
}

#[cfg(feature = "electrum")]
mod _electrum {
    use amplify::ByteArray;
    use bitcoin::hashes::Hash;
    use bitcoin::{Script, ScriptBuf};
    use bp::{Chain, LockTime, SeqNo, Tx, TxIn, TxOut, TxVer, Txid, VarIntArray, Witness};
    use electrum_client::{ElectrumApi, Error, ListUnspentRes};
    use rgbstd::contract::WitnessOrd;
    use rgbstd::resolvers::ResolveHeight;
    use rgbstd::validation::{ResolveTx, TxResolverError};

    use super::*;

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
                .transaction_get(&bitcoin::Txid::from_byte_array(txid.to_byte_array()))
                .map_err(|err| match err {
                    Error::Message(_) | Error::Protocol(_) => TxResolverError::Unknown(txid),
                    err => TxResolverError::Other(txid, err.to_string()),
                })?;
            Ok(Tx {
                version: TxVer::from_consensus_i32(tx.version),
                inputs: VarIntArray::try_from_iter(tx.input.into_iter().map(|txin| TxIn {
                    prev_output: Outpoint::new(
                        txin.previous_output.txid.to_byte_array().into(),
                        txin.previous_output.vout,
                    ),
                    sig_script: txin.script_sig.to_bytes().into(),
                    sequence: SeqNo::from_consensus_u32(txin.sequence.to_consensus_u32()),
                    witness: Witness::from_consensus_stack(txin.witness.to_vec()),
                }))
                .expect("consensus-invalid transaction"),
                outputs: VarIntArray::try_from_iter(tx.output.into_iter().map(|txout| TxOut {
                    value: txout.value.into(),
                    script_pubkey: txout.script_pubkey.to_bytes().into(),
                }))
                .expect("consensus-invalid transaction"),
                lock_time: LockTime::from_consensus_u32(tx.lock_time.to_consensus_u32()),
            })
        }
    }

    impl ResolveHeight for BlockchainResolver {
        type Error = TxResolverError;
        fn resolve_height(&mut self, txid: Txid) -> Result<WitnessOrd, Self::Error> {
            let tx = match self
                .0
                .transaction_get(&bitcoin::Txid::from_byte_array(txid.to_byte_array()))
            {
                Ok(tx) => tx,
                Err(Error::Message(_) | Error::Protocol(_)) => return Ok(WitnessOrd::OffChain),
                Err(err) => return Err(TxResolverError::Other(txid, err.to_string())),
            };

            let scripts: Vec<&Script> = tx
                .output
                .iter()
                .map(|out| out.script_pubkey.as_script())
                .collect();

            let mut hists = vec![];
            self.0
                .batch_script_get_history(scripts)
                .map_err(|err| match err {
                    Error::Message(_) | Error::Protocol(_) => TxResolverError::Unknown(txid),
                    err => TxResolverError::Other(txid, err.to_string()),
                })?
                .into_iter()
                .for_each(|h| hists.extend(h));
            let transactions: BTreeMap<bitcoin::Txid, u32> = hists
                .into_iter()
                .map(|h| (h.tx_hash, if h.height > 0 { h.height as u32 } else { 0 }))
                .collect();

            let min_height = transactions
                .into_values()
                .min()
                .map(WitnessOrd::with_mempool_or_height)
                .unwrap_or(WitnessOrd::OffChain);

            Ok(min_height)
        }
    }

    impl Utxo {
        pub fn with(derivation: DeriveInfo, res: ListUnspentRes) -> Self {
            Utxo {
                status: if res.height == 0 {
                    MiningStatus::Mempool
                } else {
                    MiningStatus::Blockchain(res.height as u32)
                },
                outpoint: Outpoint::new(
                    Txid::from_byte_array(res.tx_hash.to_byte_array()),
                    res.tx_pos as u32,
                ),
                derivation,
                amount: res.value,
            }
        }
    }
}
