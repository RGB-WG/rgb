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
#[cfg(feature = "async")]
use esplora::AsyncClient as EsploraAsyncClient;
#[cfg(not(feature = "async"))]
use esplora::BlockingClient as EsploraClient;
use esplora::{Error as EsploraError, TxStatus};
use rgb::WitnessStatus;

use super::{Resolver, ResolverError};

/// Represents the kind of client used for interacting with the Esplora indexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
enum ClientKind {
    #[default]
    Esplora,
    #[cfg(any(feature = "resolver-mempool", feature = "resolver-mempool-async"))]
    Mempool,
}

#[cfg(not(feature = "async"))]
pub struct EsploraResolver {
    inner: EsploraClient,
    kind: ClientKind,
}

#[cfg(feature = "async")]
pub struct EsploraAsyncResolver {
    inner: EsploraAsyncClient,
    kind: ClientKind,
}

fn convert_esplora_status(status: TxStatus) -> WitnessStatus {
    if !status.confirmed {
        return WitnessStatus::Tentative;
    }
    if let Some(height) = status.block_height {
        match NonZeroU64::new(height as u64) {
            Some(height) => WitnessStatus::Mined(height),
            None => WitnessStatus::Genesis,
        }
    } else {
        WitnessStatus::Archived
    }
}

#[cfg(not(feature = "async"))]
impl Resolver for EsploraResolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        let tx = self.inner.tx(&txid)?;
        Ok(tx.map(UnsignedTx::with_sigs_removed))
    }

    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        let status = self.inner.tx_status(&txid)?;
        Ok(convert_esplora_status(status))
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

    fn last_block_height(&self) -> Result<u64, ResolverError> { Ok(self.inner.height()? as u64) }

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
        self.inner.broadcast(tx)?;
        Ok(())
    }
}

#[cfg(feature = "async")]
impl Resolver for EsploraAsyncResolver {
    async fn resolve_tx_async(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        let tx = self.inner.tx(&txid).await?;
        Ok(tx.map(UnsignedTx::with_sigs_removed))
    }

    async fn resolve_tx_status_async(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        let status = self.inner.tx_status(&txid).await?;
        Ok(convert_esplora_status(status))
    }

    async fn resolve_utxos_async(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        todo!();
        iter::empty()
    }

    async fn last_block_height_async(&self) -> Result<u64, ResolverError> {
        Ok(self.inner.height().await? as u64)
    }

    async fn broadcast_async(&self, tx: &Tx) -> Result<(), ResolverError> {
        self.inner.broadcast(tx).await?;
        Ok(())
    }
}

impl From<EsploraError> for ResolverError {
    fn from(err: EsploraError) -> Self {
        match err {
            #[cfg(feature = "async")]
            EsploraError::Reqwest(_) => ResolverError::Connectivity,

            #[cfg(not(feature = "async"))]
            EsploraError::Minreq(_) => ResolverError::Connectivity,

            EsploraError::InvalidHttpHeaderName(_) | EsploraError::InvalidHttpHeaderValue(_) => {
                ResolverError::Connectivity
            }

            EsploraError::StatusCode(_)
            | EsploraError::HttpResponse { .. }
            | EsploraError::InvalidServerData
            | EsploraError::Parsing(_)
            | EsploraError::BitcoinEncoding
            | EsploraError::Hex(_) => ResolverError::Protocol,

            EsploraError::TransactionNotFound(_) => ResolverError::Logic,
        }
    }
}
