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
use rgbstd::validation::{ResolveTx, TxResolverError};
use rgbstd::{Layer1, WitnessAnchor, WitnessId, WitnessOrd, WitnessPos, XAnchor};

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

    /// invalid anchor {0}
    InvalidAnchor(String),
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

    pub fn add_terminals<const TYPE: bool>(&mut self, consignment: &Consignment<TYPE>) {
        self.terminal_txes.extend(
            consignment
                .terminals
                .values()
                .filter_map(|t| t.tx.as_ref())
                .map(|tx| (tx.txid(), tx.clone())),
        );
    }
}

impl ResolveHeight for Resolver {
    type Error = AnchorResolverError;

    fn resolve_anchor(&mut self, anchor: &XAnchor) -> Result<WitnessAnchor, Self::Error> {
        let XAnchor::Bitcoin(anchor) = anchor else {
            panic!("Liquid is not yet supported")
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
            witness_id: WitnessId::Bitcoin(txid),
        })
    }
}

impl ResolveTx for Resolver {
    fn resolve_bp_tx(&self, layer1: Layer1, txid: Txid) -> Result<Tx, TxResolverError> {
        assert_eq!(layer1, Layer1::Bitcoin, "Liquid is not yet supported");

        if let Some(tx) = self.terminal_txes.get(&txid) {
            return Ok(tx.clone());
        }

        self.esplora_client
            .tx(&txid)
            .map_err(|err| TxResolverError::Other(txid, err.to_string()))?
            .ok_or(TxResolverError::Unknown(txid))
    }
}
