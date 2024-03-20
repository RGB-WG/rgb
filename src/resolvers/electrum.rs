// RGB smart contracts for Bitcoin & Lightning
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2024 by
//     Zoe Faltibà <zoefaltiba@gmail.com>
//
// Copyright (C) 2024 LNP/BP Standards Association. All rights reserved.
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

use std::collections::HashMap;

use bp::ConsensusDecode;
use bpstd::{Tx, Txid};
use electrum::{Client, ElectrumApi, Error, Param};
use rgbstd::containers::Consignment;
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{ResolveWitness, WitnessResolverError};
use rgbstd::{Layer1, WitnessAnchor, WitnessId, WitnessOrd, WitnessPos, XAnchor, XPubWitness};

pub struct Resolver {
    electrum_client: Client,
    terminal_txes: HashMap<Txid, Tx>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum AnchorResolverError {
    #[from]
    #[display(inner)]
    Error(Error),

    /// impossible conversion
    ImpossibleConversion,

    /// invalid anchor {0}
    InvalidAnchor(String),

    /// unsupported layer 1 {0}
    UnsupportedLayer1(Layer1),
}

impl Resolver {
    #[allow(clippy::result_large_err)]
    pub fn new(url: &str) -> Result<Self, Error> {
        let electrum_client = Client::new(url)?;
        Ok(Self {
            electrum_client,
            terminal_txes: none!(),
        })
    }

    pub fn add_terminals<const TYPE: bool>(&mut self, consignment: &Consignment<TYPE>) {
        self.terminal_txes.extend(
            consignment
                .terminals
                .values()
                .filter_map(|t| t.witness_tx.as_ref().map(XPubWitness::as_reduced_unsafe))
                .map(|tx| (tx.txid(), tx.clone())),
        );
    }
}

impl ResolveHeight for Resolver {
    type Error = AnchorResolverError;

    fn resolve_anchor(&mut self, anchor: &XAnchor) -> Result<WitnessAnchor, Self::Error> {
        let XAnchor::Bitcoin(anchor) = anchor else {
            return Err(AnchorResolverError::UnsupportedLayer1(anchor.layer1()));
        };
        let txid = anchor
            .txid()
            .ok_or(AnchorResolverError::InvalidAnchor(format!("{:#?}", anchor)))?;

        if self.terminal_txes.contains_key(&txid) {
            return Ok(WitnessAnchor {
                witness_ord: WitnessOrd::OffChain,
                witness_id: WitnessId::Bitcoin(txid),
            });
        }

        fn get_block_height(electrum_client: &Client) -> Result<u64, AnchorResolverError> {
            electrum_client
                .block_headers_subscribe()?
                .height
                .try_into()
                .map_err(|_| AnchorResolverError::ImpossibleConversion)
        }

        let last_block_height_min = get_block_height(&self.electrum_client)?;
        let witness_ord = match self
            .electrum_client
            .raw_call("blockchain.transaction.get", vec![
                Param::String(txid.to_string()),
                Param::Bool(true),
            ]) {
            Ok(tx_details) => {
                if let Some(confirmations) = tx_details.get("confirmations") {
                    let confirmations = confirmations
                        .as_u64()
                        .ok_or(Error::InvalidResponse(tx_details.clone()))?;
                    let last_block_height_max = get_block_height(&self.electrum_client)?;
                    let skew = confirmations - 1;
                    let mut tx_height: u32 = 0;
                    for height in (last_block_height_min - skew)..=(last_block_height_max - skew) {
                        if let Ok(get_merkle_res) = self.electrum_client.transaction_get_merkle(
                            &txid,
                            height
                                .try_into()
                                .map_err(|_| AnchorResolverError::ImpossibleConversion)?,
                        ) {
                            tx_height = get_merkle_res
                                .block_height
                                .try_into()
                                .map_err(|_| AnchorResolverError::ImpossibleConversion)?;
                            break;
                        } else {
                            continue;
                        }
                    }
                    let block_time = tx_details
                        .get("blocktime")
                        .ok_or(Error::InvalidResponse(tx_details.clone()))?
                        .as_i64()
                        .ok_or(Error::InvalidResponse(tx_details.clone()))?;
                    WitnessOrd::OnChain(
                        WitnessPos::new(tx_height, block_time)
                            .ok_or(Error::InvalidResponse(tx_details.clone()))?,
                    )
                } else {
                    WitnessOrd::OffChain
                }
            }
            Err(e)
                if e.to_string()
                    .contains("No such mempool or blockchain transaction") =>
            {
                WitnessOrd::OffChain
            }
            Err(e) => return Err(e.into()),
        };

        Ok(WitnessAnchor {
            witness_ord,
            witness_id: WitnessId::Bitcoin(txid),
        })
    }
}

impl ResolveWitness for Resolver {
    fn resolve_pub_witness(
        &self,
        witness_id: WitnessId,
    ) -> Result<XPubWitness, WitnessResolverError> {
        let WitnessId::Bitcoin(txid) = witness_id else {
            return Err(WitnessResolverError::Other(
                witness_id,
                AnchorResolverError::UnsupportedLayer1(witness_id.layer1()).to_string(),
            ));
        };

        if let Some(tx) = self.terminal_txes.get(&txid) {
            return Ok(XPubWitness::Bitcoin(tx.clone()));
        }

        match self.electrum_client.transaction_get_raw(&txid) {
            Ok(raw_tx) => {
                let tx = Tx::consensus_deserialize(raw_tx).map_err(|_| {
                    WitnessResolverError::Other(witness_id, s!("cannot deserialize raw TX"))
                })?;
                Ok(XPubWitness::Bitcoin(tx))
            }
            Err(e)
                if e.to_string()
                    .contains("No such mempool or blockchain transaction") =>
            {
                Err(WitnessResolverError::Unknown(witness_id))
            }
            Err(e) => Err(WitnessResolverError::Other(witness_id, e.to_string())),
        }
    }
}
