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

use core::num::NonZeroU64;

use bpstd::psbt::Utxo;
use bpstd::{Sats, ScriptPubkey, Terminal, Tx, Txid, UnsignedTx, Vout};
#[cfg(feature = "async")]
use esplora::AsyncClient as EsploraAsyncClient;
#[cfg(not(feature = "async"))]
use esplora::BlockingClient as EsploraClient;
use esplora::{Error as EsploraError, TxStatus};
use rgb::{Outpoint, WitnessStatus};

use super::{Resolver, ResolverError};

#[cfg(not(feature = "async"))]
pub struct EsploraResolver(EsploraClient);

#[cfg(feature = "async")]
pub struct EsploraAsyncResolver(EsploraAsyncClient);

#[cfg(not(feature = "async"))]
impl EsploraResolver {
    /// Creates a new Esplora client with the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the Esplora server.
    ///
    /// # Errors
    ///
    /// Returns an error if the client fails to connect to the Esplora server.
    pub fn new(url: &str) -> Result<Self, ResolverError> {
        let inner = esplora::Builder::new(url).build_blocking()?;
        let client = Self(inner);
        Ok(client)
    }
}

#[cfg(feature = "async")]
impl EsploraAsyncResolver {
    /// Creates a new Esplora client with the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the Esplora server.
    ///
    /// # Errors
    ///
    /// Returns an error if the client fails to connect to the Esplora server.
    pub fn new(url: &str) -> Result<Self, ResolverError> {
        let inner = esplora::Builder::new(url).build_async()?;
        let client = Self(inner);
        Ok(client)
    }
}

fn convert_status(status: TxStatus) -> WitnessStatus {
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
        let tx = self.0.tx(&txid)?;
        Ok(tx.map(UnsignedTx::with_sigs_removed))
    }

    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        let status = self.0.tx_status(&txid)?;
        Ok(convert_status(status))
    }

    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        iter.into_iter()
            .flat_map(|(terminal, spk)| match self.0.scripthash_utxo(&spk) {
                Err(err) => vec![Err(ResolverError::from(err))],
                Ok(list) => list
                    .into_iter()
                    .map(|utxo| {
                        Ok(Utxo {
                            outpoint: Outpoint::new(
                                utxo.txid,
                                Vout::from_u32(utxo.vout.value as u32),
                            ),
                            value: Sats::from_sats(utxo.value),
                            terminal,
                        })
                    })
                    .collect::<Vec<_>>(),
            })
    }

    fn last_block_height(&self) -> Result<u64, ResolverError> { Ok(self.0.height()? as u64) }

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
        self.0.broadcast(tx)?;
        Ok(())
    }
}

#[cfg(feature = "async")]
impl Resolver for EsploraAsyncResolver {
    async fn resolve_tx_async(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        let tx = self.0.tx(&txid).await?;
        Ok(tx.map(UnsignedTx::with_sigs_removed))
    }

    async fn resolve_tx_status_async(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        let status = self.0.tx_status(&txid).await?;
        Ok(convert_status(status))
    }

    async fn resolve_utxos_async(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        let mut utxos = Vec::new();
        for (terminal, spk) in iter {
            match self.0.scripthash_utxo(&spk).await {
                Err(err) => utxos.push(Err(ResolverError::from(err))),
                Ok(list) => utxos.extend(list.into_iter().map(|utxo| {
                    Ok(Utxo {
                        outpoint: Outpoint::new(utxo.txid, Vout::from_u32(utxo.vout.value as u32)),
                        value: Sats::from_sats(utxo.value),
                        terminal,
                    })
                })),
            }
        }
        utxos.into_iter()
    }

    async fn last_block_height_async(&self) -> Result<u64, ResolverError> {
        Ok(self.0.height().await? as u64)
    }

    async fn broadcast_async(&self, tx: &Tx) -> Result<(), ResolverError> {
        self.0.broadcast(tx).await?;
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
