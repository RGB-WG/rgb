// RGB smart contracts for Bitcoin & Lightning
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2024 by
//     Zoe Faltibà <zoefaltiba@gmail.com>
// Rewritten in 2024 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
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

use bp::Tx;
use rgbstd::containers::Consignment;
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{ResolveWitness, WitnessResolverError};
use rgbstd::{WitnessAnchor, XWitnessId, XWitnessTx};

use crate::{Txid, WitnessOrd, XChain};

pub trait RgbResolver {
    fn resolve_height(&mut self, txid: Txid) -> Result<WitnessAnchor, String>;
    fn resolve_pub_witness(&self, txid: Txid) -> Result<Tx, Option<String>>;
}

/// Type that contains any of the [`Resolver`] types defined by the library
#[derive(From)]
#[non_exhaustive]
pub struct AnyResolver {
    inner: Box<dyn RgbResolver>,
    terminal_txes: HashMap<Txid, Tx>,
}

impl AnyResolver {
    #[cfg(feature = "electrum_blocking")]
    pub fn electrum_blocking(url: &str) -> Result<Self, String> {
        Ok(AnyResolver {
            inner: Box::new(electrum::Client::new(url).map_err(|e| e.to_string())?),
            terminal_txes: Default::default(),
        })
    }

    #[cfg(feature = "esplora_blocking")]
    pub fn esplora_blocking(url: &str) -> Result<Self, String> {
        Ok(AnyResolver {
            inner: Box::new(
                esplora::Builder::new(url)
                    .build_blocking()
                    .map_err(|e| e.to_string())?,
            ),
            terminal_txes: Default::default(),
        })
    }

    pub fn add_terminals<const TYPE: bool>(&mut self, consignment: &Consignment<TYPE>) {
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

impl ResolveHeight for AnyResolver {
    fn resolve_height(&mut self, witness_id: XWitnessId) -> Result<WitnessAnchor, String> {
        let XWitnessId::Bitcoin(txid) = witness_id else {
            return Err(format!("{} is not supported as layer 1 network", witness_id.layer1()));
        };

        if self.terminal_txes.contains_key(&txid) {
            return Ok(WitnessAnchor {
                witness_ord: WitnessOrd::OffChain,
                witness_id,
            });
        }

        self.inner.resolve_height(txid)
    }
}

impl ResolveWitness for AnyResolver {
    fn resolve_pub_witness(
        &self,
        witness_id: XWitnessId,
    ) -> Result<XWitnessTx, WitnessResolverError> {
        let XWitnessId::Bitcoin(txid) = witness_id else {
            return Err(WitnessResolverError::Other(
                witness_id,
                format!("{} is not supported as layer 1 network", witness_id.layer1()),
            ));
        };

        if let Some(tx) = self.terminal_txes.get(&txid) {
            return Ok(XWitnessTx::Bitcoin(tx.clone()));
        }

        self.inner
            .resolve_pub_witness(txid)
            .map(XWitnessTx::Bitcoin)
            .map_err(|e| match e {
                None => WitnessResolverError::Unknown(witness_id),
                Some(e) => WitnessResolverError::Other(witness_id, e),
            })
    }
}
