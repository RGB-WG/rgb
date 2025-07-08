// Wallet Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
// Copyright (C) 2025 RGB Consortium, Switzerland.
// Copyright (C) 2019-2025 Dr Maxim Orlovsky.
// All rights under the above copyrights are reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use std::iter;
use std::num::NonZeroU64;

use bpstd::psbt::Utxo;
use bpstd::{ScriptPubkey, Terminal, Tx, Txid, UnsignedTx};
use esplora::{BlockingClient as EsploraClient, Error, Error as EsploraError};
use rgb::WitnessStatus;

use crate::resolvers::ResolverError;
use crate::Resolver;

/// Represents the kind of client used for interacting with the Esplora indexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
enum ClientKind {
    #[default]
    Esplora,
    #[cfg(feature = "resolver-mempool")]
    Mempool,
}

pub struct EsploraResolver {
    inner: EsploraClient,
    kind: ClientKind,
}

impl Resolver for EsploraResolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        let tx = self.inner.tx(&txid)?;
        Ok(tx.map(UnsignedTx::with_sigs_removed))
    }

    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        let status = self.inner.tx_status(&txid)?;
        if !status.confirmed {
            return Ok(WitnessStatus::Tentative);
        }
        if let Some(height) = status.block_height {
            Ok(match NonZeroU64::new(height as u64) {
                Some(height) => WitnessStatus::Mined(height),
                None => WitnessStatus::Genesis,
            })
        } else {
            Ok(WitnessStatus::Archived)
        }
    }

    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        todo!();
        /*
        const PAGE_SIZE: usize = 25;

        let mut res = Vec::new();
        let mut last_seen = None;
        let script = derive.addr.script_pubkey();

        loop {
            let r = match self.kind {
                ClientKind::Esplora => self.inner.scripthash_txs(&script, last_seen)?,
                #[cfg(feature = "resolver-mempool")]
                ClientKind::Mempool => self.inner.address_txs(&derive.addr, last_seen)?,
            };
            match &r[..] {
                [a @ .., esplora::Tx { txid, .. }] if a.len() >= PAGE_SIZE - 1 => {
                    last_seen = Some(*txid);
                    res.extend(r);
                }
                _ => {
                    res.extend(r);
                    break;
                }
            }
        }

        Ok(res)
         */
        iter::empty()
    }

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
        self.inner.broadcast(tx)?;
        Ok(())
    }
}

impl From<EsploraError> for ResolverError {
    fn from(err: EsploraError) -> Self {
        match err {
            Error::Minreq(_)
            | Error::InvalidHttpHeaderName(_)
            | Error::InvalidHttpHeaderValue(_) => ResolverError::Connectivity,

            Error::StatusCode(_)
            | Error::HttpResponse { .. }
            | Error::InvalidServerData
            | Error::Parsing(_)
            | Error::BitcoinEncoding
            | Error::Hex(_) => ResolverError::Protocol,

            Error::TransactionNotFound(_) => ResolverError::Logic,
        }
    }
}
