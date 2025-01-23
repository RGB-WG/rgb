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

use bp::{Tx, Txid};
use bpstd::Network;
use rgbstd::containers::Consignment;
use rgbstd::validation::{ResolveWitness, WitnessResolverError};

use crate::vm::WitnessOrd;

// We need to repeat methods of `WitnessResolve` trait here to avoid making
// wrappers around resolver types. TODO: Use wrappers instead
pub trait RgbResolver: Send {
    fn check(&self, network: Network, expected_block_hash: String) -> Result<(), String>;
    fn resolve_pub_witness(&self, txid: Txid) -> Result<Option<Tx>, String>;
    fn resolve_pub_witness_ord(&self, txid: Txid) -> Result<WitnessOrd, String>;
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
    pub fn electrum_blocking(url: &str, config: Option<electrum::Config>) -> Result<Self, String> {
        Ok(AnyResolver {
            inner: Box::new(
                electrum::Client::from_config(url, config.unwrap_or_default())
                    .map_err(|e| e.to_string())?,
            ),
            terminal_txes: Default::default(),
        })
    }

    #[cfg(feature = "esplora_blocking")]
    pub fn esplora_blocking(url: &str, config: Option<esplora::Config>) -> Result<Self, String> {
        Ok(AnyResolver {
            inner: Box::new(
                esplora::BlockingClient::from_config(url, config.unwrap_or_default())
                    .map_err(|e| e.to_string())?,
            ),
            terminal_txes: Default::default(),
        })
    }

    #[cfg(feature = "mempool_blocking")]
    pub fn mempool_blocking(url: &str, config: Option<esplora::Config>) -> Result<Self, String> {
        Ok(AnyResolver {
            inner: Box::new(super::mempool_blocking::MemPoolClient::new(
                url,
                config.unwrap_or_default(),
            )?),
            terminal_txes: Default::default(),
        })
    }
    pub fn check(&self, network: Network) -> Result<(), String> {
        let expected_block_hash = match network {
            Network::Mainnet => "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f",
            Network::Testnet3 => "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943",
            Network::Testnet4 => "00000000da84f2bafbbc53dee25a72ae507ff4914b867c565be350b0da8bf043",
            Network::Signet => "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6",
            Network::Regtest => "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
        }
        .to_string();
        self.inner.check(network, expected_block_hash)
    }

    pub fn add_terminals<const TYPE: bool>(&mut self, consignment: &Consignment<TYPE>) {
        self.terminal_txes.extend(
            consignment
                .bundles
                .iter()
                .filter_map(|bw| bw.pub_witness.tx().cloned())
                .map(|tx| (tx.txid(), tx)),
        );
    }
}

impl ResolveWitness for AnyResolver {
    fn resolve_pub_witness(&self, witness_id: Txid) -> Result<Tx, WitnessResolverError> {
        if let Some(tx) = self.terminal_txes.get(&witness_id) {
            return Ok(tx.clone());
        }

        self.inner
            .resolve_pub_witness(witness_id)
            .map_err(|e| WitnessResolverError::Other(witness_id, e))
            .and_then(|r| r.ok_or(WitnessResolverError::Unknown(witness_id)))
    }

    fn resolve_pub_witness_ord(
        &self,
        witness_id: Txid,
    ) -> Result<WitnessOrd, WitnessResolverError> {
        if self.terminal_txes.contains_key(&witness_id) {
            return Ok(WitnessOrd::Tentative);
        }

        self.inner
            .resolve_pub_witness_ord(witness_id)
            .map_err(|e| WitnessResolverError::Other(witness_id, e))
    }
}
