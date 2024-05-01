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

use bp::ConsensusDecode;
use bpstd::{Tx, Txid};
use electrum::{Client, ElectrumApi, Error, Param};
use rgbstd::{WitnessAnchor, WitnessOrd, WitnessPos, XWitnessId};

use super::RgbResolver;

impl RgbResolver for Client {
    fn resolve_height(&mut self, txid: Txid) -> Result<WitnessAnchor, String> {
        let mut witness_anchor = WitnessAnchor {
            witness_ord: WitnessOrd::OffChain,
            witness_id: XWitnessId::Bitcoin(txid),
        };

        let tx_details = self
            .raw_call("blockchain.transaction.get", vec![
                Param::String(txid.to_string()),
                Param::Bool(true),
            ])
            .map_err(|e| e.to_string())?;

        let Some(confirmations) = tx_details.get("confirmations") else {
            return Ok(witness_anchor);
        };
        let confirmations = confirmations
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .ok_or(Error::InvalidResponse(tx_details.clone()))
            .map_err(|e| e.to_string())?;

        let header = self.block_headers_subscribe().map_err(|e| e.to_string())?;
        let last_block_height_min =
            u32::try_from(header.height).map_err(|_| s!("height overflow"))?;
        let last_block_height_max = last_block_height_min;
        let skew = confirmations - 1;
        let mut tx_height: u32 = 0;
        for height in (last_block_height_min - skew)..=(last_block_height_max - skew) {
            if let Ok(get_merkle_res) = self.transaction_get_merkle(&txid, height as usize) {
                tx_height = u32::try_from(get_merkle_res.block_height)
                    .map_err(|_| s!("height overflow"))?;
                break;
            }
        }
        let block_time = tx_details
            .get("blocktime")
            .and_then(|v| v.as_i64())
            .ok_or(Error::InvalidResponse(tx_details.clone()))
            .map_err(|e| e.to_string())?;
        witness_anchor.witness_ord = WitnessOrd::OnChain(
            WitnessPos::new(tx_height, block_time)
                .ok_or(Error::InvalidResponse(tx_details.clone()))
                .map_err(|e| e.to_string())?,
        );

        Ok(witness_anchor)
    }

    fn resolve_pub_witness(&self, txid: Txid) -> Result<Tx, Option<String>> {
        let raw_tx = self.transaction_get_raw(&txid).map_err(|e| {
            let e = e.to_string();
            if e.contains("No such mempool or blockchain transaction") {
                return None;
            }
            Some(e)
        })?;
        Tx::consensus_deserialize(raw_tx)
            .map_err(|e| Some(format!("cannot deserialize raw TX - {e}")))
    }
}
