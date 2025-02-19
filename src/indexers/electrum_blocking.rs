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

use std::iter;
use std::num::NonZeroU32;

use bp::ConsensusDecode;
use bpstd::{Tx, Txid};
use electrum::{Client, ElectrumApi, Param};
pub use electrum::{Config, ConfigBuilder, Error, Socks5Config};
use rgbstd::vm::WitnessPos;
use rgbstd::ChainNet;

use super::RgbResolver;
use crate::vm::WitnessOrd;

macro_rules! check {
    ($e:expr) => {
        $e.map_err(|e| e.to_string())?
    };
}

impl RgbResolver for Client {
    fn check_chain_net(&self, chain_net: ChainNet) -> Result<(), String> {
        // check the electrum server is for the correct network
        let block_hash = check!(self.block_header(0)).block_hash();
        if chain_net.genesis_block_hash() != block_hash {
            return Err(s!("resolver is for a network different from the wallet's one"));
        }
        // check the electrum server has the required functionality (verbose
        // transactions)
        let txid = match chain_net {
            ChainNet::BitcoinMainnet => {
                "33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036"
            }
            ChainNet::BitcoinTestnet3 => {
                "5e6560fd518aadbed67ee4a55bdc09f19e619544f5511e9343ebba66d2f62653"
            }
            ChainNet::BitcoinTestnet4 => {
                "7aa0a7ae1e223414cb807e40cd57e667b718e42aaf9306db9102fe28912b7b4e"
            }
            ChainNet::BitcoinSignet => {
                "8153034f45e695453250a8fb7225a5e545144071d8ed7b0d3211efa1f3c92ad8"
            }
            ChainNet::BitcoinRegtest => {
                "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
            }
            _ => return Err(s!("only bitcoin is supported")),
        };
        if let Err(e) = self.raw_call("blockchain.transaction.get", vec![
            Param::String(txid.to_string()),
            Param::Bool(true),
        ]) {
            if !e
                .to_string()
                .contains("genesis block coinbase is not considered an ordinary transaction")
            {
                return Err(s!(
                    "verbose transactions are unsupported by the provided electrum service"
                ));
            }
        }
        Ok(())
    }

    fn resolve_pub_witness_ord(&self, txid: Txid) -> Result<WitnessOrd, String> {
        // We get the height of the tip of blockchain
        let header = check!(self.block_headers_subscribe());

        // Now we get and parse transaction information to get the number of
        // confirmations
        let tx_details = match self.raw_call("blockchain.transaction.get", vec![
            Param::String(txid.to_string()),
            Param::Bool(true),
        ]) {
            Err(e)
                if e.to_string()
                    .contains("No such mempool or blockchain transaction") =>
            {
                return Ok(WitnessOrd::Archived);
            }
            Err(e) => return Err(e.to_string()),
            Ok(v) => v,
        };
        let forward = iter::from_fn(|| self.block_headers_pop().ok().flatten()).count() as isize;

        let Some(confirmations) = tx_details.get("confirmations") else {
            return Ok(WitnessOrd::Tentative);
        };
        let confirmations = check!(confirmations
            .as_u64()
            .and_then(|x| u32::try_from(x).ok())
            .ok_or(Error::InvalidResponse(tx_details.clone())));
        if confirmations == 0 {
            return Ok(WitnessOrd::Tentative);
        }
        let block_time = check!(tx_details
            .get("blocktime")
            .and_then(|v| v.as_i64())
            .ok_or(Error::InvalidResponse(tx_details.clone())));

        let tip_height = u32::try_from(header.height).map_err(|_| s!("impossible height value"))?;
        let height: isize = (tip_height - confirmations) as isize;
        const SAFETY_MARGIN: isize = 1;
        // first check from expected min to max height
        let get_merkle_res = (1..=forward + 1)
            // we need this under assumption that electrum was lying due to "DB desynchronization"
            // since this have a very low probability we do that after everything else
            .chain((1..=SAFETY_MARGIN).flat_map(|i| [i + forward + 1, 1 - i]))
            .find_map(|offset| self.transaction_get_merkle(&txid, (height + offset) as usize).ok())
            .ok_or_else(|| s!("transaction can't be located in the blockchain"))?;

        let tx_height = u32::try_from(get_merkle_res.block_height)
            .map_err(|_| s!("impossible height value"))?;

        let height =
            check!(NonZeroU32::new(tx_height).ok_or(Error::InvalidResponse(tx_details.clone())));
        let pos = check!(WitnessPos::bitcoin(height, block_time)
            .ok_or(Error::InvalidResponse(tx_details.clone())));

        Ok(WitnessOrd::Mined(pos))
    }

    fn resolve_pub_witness(&self, txid: Txid) -> Result<Option<Tx>, String> {
        self.transaction_get_raw(&txid)
            .map_err(|e| e.to_string())
            .and_then(|raw_tx| {
                Tx::consensus_deserialize(raw_tx)
                    .map_err(|e| format!("cannot deserialize raw TX - {e}"))
            })
            .map(Some)
            .or_else(|e| {
                if e.contains("No such mempool or blockchain transaction") {
                    Ok(None)
                } else {
                    Err(e)
                }
            })
    }
}
