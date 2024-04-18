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

use std::collections::HashMap;

use bpstd::{Tx, Txid};
pub use esplora::Error as ResolverError;
use rgbstd::containers::Consignment;
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{ResolveWitness, WitnessResolverError};
use rgbstd::{Layer1, WitnessAnchor, WitnessOrd, WitnessPos, XChain, XWitnessId, XWitnessTx};

pub struct Resolver {
    esplora_client: esplora::BlockingClient,
    terminal_txes: HashMap<Txid, Tx>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum AnchorResolverError {
    #[from]
    #[display(inner)]
    Error(esplora::Error),

    /// unsupported layer 1 {0}
    UnsupportedLayer1(Layer1),
}

impl Resolver {
    #[allow(clippy::result_large_err)]
    pub fn new(url: &str) -> Result<Self, ResolverError> {
        let esplora_client = esplora::Builder::new(url).build_blocking()?;
        Ok(Self {
            esplora_client,
            terminal_txes: none!(),
        })
    }

    pub fn add_witnesses<const TYPE: bool>(&mut self, consignment: &Consignment<TYPE>) {
        self.terminal_txes.extend(
            consignment
                .bundles
                .iter()
                .filter_map(|bw| bw.pub_witness.maybe_map_ref(|w| w.tx.clone()))
                .filter_map(|tx| match tx {
                    XChain::Bitcoin(tx) => Some(tx),
                    XChain::Liquid(_) | XChain::Other(_) => None,
                })
                .map(|tx| (tx.txid(), tx)),
        );
    }
}

impl ResolveHeight for Resolver {
    type Error = AnchorResolverError;

    fn resolve_height(&mut self, witness_id: XWitnessId) -> Result<WitnessAnchor, Self::Error> {
        let XWitnessId::Bitcoin(txid) = witness_id else {
            return Err(AnchorResolverError::UnsupportedLayer1(witness_id.layer1()));
        };

        if self.terminal_txes.contains_key(&txid) {
            return Ok(WitnessAnchor {
                witness_ord: WitnessOrd::OffChain,
                witness_id,
            });
        }

        let status = self.esplora_client.tx_status(&txid)?;
        let ord = match status
            .block_height
            .and_then(|h| status.block_time.map(|t| (h, t)))
        {
            Some((h, t)) => WitnessOrd::OnChain(
                WitnessPos::new(h, t as i64).ok_or(esplora::Error::InvalidServerData)?,
            ),
            None => WitnessOrd::OffChain,
        };
        Ok(WitnessAnchor {
            witness_ord: ord,
            witness_id,
        })
    }
}

impl ResolveWitness for Resolver {
    fn resolve_pub_witness(
        &self,
        witness_id: XWitnessId,
    ) -> Result<XWitnessTx, WitnessResolverError> {
        let XWitnessId::Bitcoin(txid) = witness_id else {
            return Err(WitnessResolverError::Other(
                witness_id,
                AnchorResolverError::UnsupportedLayer1(witness_id.layer1()).to_string(),
            ));
        };

        if let Some(tx) = self.terminal_txes.get(&txid) {
            return Ok(XWitnessTx::Bitcoin(tx.clone()));
        }

        self.esplora_client
            .tx(&txid)
            .map_err(|err| WitnessResolverError::Other(witness_id, err.to_string()))?
            .ok_or(WitnessResolverError::Unknown(witness_id))
            .map(XChain::Bitcoin)
    }
}
