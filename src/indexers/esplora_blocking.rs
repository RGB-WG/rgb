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

use bp::Tx;
use bpstd::{Network, Txid};
use esplora::{BlockingClient, Error};
use rgbstd::vm::WitnessPos;

use super::RgbResolver;
use crate::vm::WitnessOrd;

impl RgbResolver for BlockingClient {
    fn check(&self, _network: Network, expected_block_hash: String) -> Result<(), String> {
        // check the esplora server is for the correct network
        let block_hash = self.block_hash(0)?.to_string();
        if expected_block_hash != block_hash {
            return Err(s!("resolver is for a network different from the wallet's one"));
        }
        Ok(())
    }

    fn resolve_pub_witness_ord(&self, txid: Txid) -> Result<WitnessOrd, String> {
        let status = self.tx_status(&txid)?;
        let ord = match status
            .block_height
            .and_then(|h| status.block_time.map(|t| (h, t)))
        {
            Some((h, t)) => {
                WitnessOrd::Mined(WitnessPos::new(h, t as i64).ok_or(Error::InvalidServerData)?)
            }
            // TODO: Figure out how to detect mempool transactions
            None => WitnessOrd::Archived,
        };
        Ok(ord)
    }

    fn resolve_pub_witness(&self, txid: Txid) -> Result<Option<Tx>, String> {
        self.tx(&txid).or_else(|e| match e {
            Error::TransactionNotFound(_) => Ok(None),
            e => Err(e.to_string()),
        })
    }
}
