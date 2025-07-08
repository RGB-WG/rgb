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

use std::num::NonZeroU64;

use bpstd::psbt::Utxo;
use bpstd::{Outpoint, Sats, ScriptPubkey, Terminal, Tx, Txid, UnsignedTx};
use electrum::client::Client as ElectrumClient;
use electrum::{ElectrumApi, Error as ElectrumError, Error};
use rgb::WitnessStatus;

use crate::resolvers::ResolverError;
use crate::Resolver;

pub struct ElectrumResolver(ElectrumClient);

impl Resolver for ElectrumResolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        let tx = self.0.transaction_get(&txid)?;
        Ok(tx.map(UnsignedTx::with_sigs_removed))
    }

    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        let Some(verbose) = self.0.transaction_get_verbose(&txid)? else {
            return Ok(WitnessStatus::Archived);
        };
        if verbose.block_hash.is_none() {
            return Ok(WitnessStatus::Tentative);
        };
        if verbose.time.is_none() {
            return Ok(WitnessStatus::Tentative);
        };
        let last_header = self.0.block_headers_subscribe()?;
        let height = last_header.height as u64 - verbose.confirmations as u64;
        let Some(height) = NonZeroU64::new(height) else {
            return Ok(WitnessStatus::Genesis);
        };
        Ok(WitnessStatus::Mined(height))
    }

    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        iter.into_iter()
            .flat_map(|(terminal, spk)| match self.0.script_list_unspent(&spk) {
                Err(err) => vec![Err(ResolverError::from(err))],
                Ok(list) => list
                    .into_iter()
                    .map(|res| {
                        Ok(Utxo {
                            outpoint: Outpoint::new(res.tx_hash, res.tx_pos as u32),
                            value: Sats::from_sats(res.value),
                            terminal,
                        })
                    })
                    .collect::<Vec<_>>(),
            })
    }

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
        self.0.transaction_broadcast(tx)?;
        Ok(())
    }
}

impl From<ElectrumError> for ResolverError {
    fn from(err: ElectrumError) -> Self {
        match err {
            Error::IOError(err) => ResolverError::Io(err.into()),
            Error::SharedIOError(err) => ResolverError::Io(err.kind().into()),

            Error::InvalidDNSNameError(_) | Error::MissingDomain => ResolverError::Connectivity,

            Error::CouldNotCreateConnection(_) | Error::CouldntLockReader | Error::Mpsc => {
                ResolverError::Local
            }

            Error::InvalidResponse(_)
            | Error::JSON(_)
            | Error::Hex(_)
            | Error::JSONRpc(_)
            | Error::Bitcoin(_) => ResolverError::Protocol,

            Error::Protocol(err) => ResolverError::ServerSide(err.message),

            Error::AlreadySubscribed(_) | Error::NotSubscribed(_) => ResolverError::Logic,

            Error::AllAttemptsErrored(list) => list
                .into_iter()
                .next()
                .map(ResolverError::from)
                .unwrap_or(ResolverError::Protocol),
        }
    }
}
