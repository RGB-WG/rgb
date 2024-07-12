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

use std::io::Cursor;

use bp::{ConsensusDecode, Tx};
use bpstd::{Network, Txid};
use esplora::{Error, TxStatus};
use rgbstd::{WitnessAnchor, WitnessOrd, WitnessPos};

use super::RgbResolver;
use crate::XWitnessId;

#[derive(Clone, Debug)]
/// Represents a client for interacting with a mempool.
pub struct MemPoolClient {
    url: String,
}

/// Represents a client for interacting with a mempool.
impl MemPoolClient {
    /// Creates a new instance of `MemPoolClient` with the specified URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the mempool.
    ///
    /// # Returns
    ///
    /// A new instance of `MemPoolClient`.
    pub fn new(url: &str) -> Self {
        MemPoolClient {
            url: url.to_string(),
        }
    }
}

impl RgbResolver for MemPoolClient {
    fn check(&self, _network: Network, expected_block_hash: String) -> Result<(), String> {
        // check the mempool server is for the correct network
        let block_hash = self.block_hash(0)?;
        if expected_block_hash != block_hash {
            return Err(s!("resolver is for a network different from the wallet's one"));
        }
        Ok(())
    }

    fn resolve_height(&mut self, txid: Txid) -> Result<WitnessAnchor, String> {
        let status = self.tx_status(&txid)?;
        let ord = match status
            .block_height
            .and_then(|h| status.block_time.map(|t| (h, t)))
        {
            Some((h, t)) => {
                WitnessOrd::OnChain(WitnessPos::new(h, t as i64).ok_or(Error::InvalidServerData)?)
            }
            None => WitnessOrd::OffChain,
        };
        Ok(WitnessAnchor {
            witness_ord: ord,
            witness_id: XWitnessId::Bitcoin(txid),
        })
    }

    fn resolve_pub_witness(&self, txid: Txid) -> Result<Tx, Option<String>> {
        self.tx(&txid).map_err(|e| {
            if e.contains("Transaction not found") {
                None
            } else {
                Some(e)
            }
        })
    }
}

/// Implementation of a MemPoolClient struct.
impl MemPoolClient {
    /// Retrieves the block hash for a given height from mempool.space.
    ///
    /// # Arguments
    ///
    /// * `height` - The height of the block.
    ///
    /// # Returns
    ///
    /// Returns a Result containing the block hash as a String if successful, or an error message as a String if unsuccessful.
    fn block_hash(&self, height: u32) -> Result<String, String> {
        let url = self.url.as_str();
        let http_response = reqwest::blocking::get(format!("{url}/block-height/{height}"))
            .map_err(|err| format!("Failed to get block-hash from mempool.space: {}", err))?;
        let response = http_response
            .text()
            .map_err(|err| format!("Failed to get block-hash from mempool.space: {}", err))?;
        Ok(response)
    }

    /// Retrieves the transaction status for a given transaction ID from mempool.space.
    ///
    /// # Arguments
    ///
    /// * `txid` - The transaction ID.
    ///
    /// # Returns
    ///
    /// Returns a Result containing the transaction status as a TxStatus enum if successful, or an error message as a String if unsuccessful.
    fn tx_status(&self, txid: &Txid) -> Result<TxStatus, String> {
        let url = self.url.as_str();
        let http_response = reqwest::blocking::get(format!("{url}/tx/{txid}/status"))
            .map_err(|err| format!("Failed to get tx status from mempool.space: {}", err))?;
        let response = http_response
            .json::<TxStatus>()
            .map_err(|err| format!("Failed to get tx status from mempool.space: {}", err))?;
        Ok(response)
    }

    /// Retrieves the transaction for a given transaction ID from mempool.space.
    ///
    /// # Arguments
    ///
    /// * `txid` - The transaction ID.
    ///
    /// # Returns
    ///
    /// Returns a Result containing the transaction as a Tx struct if successful, or an error message as a String if unsuccessful.
    fn tx(&self, txid: &Txid) -> Result<Tx, String> {
        let url = self.url.as_str();
        let http_response = reqwest::blocking::get(format!("{url}/tx/{txid}/raw"))
            .map_err(|err| format!("Failed to get tx from mempool.space: {}", err))?;
        let bytes = http_response
            .bytes()
            .map_err(|err| format!("Failed to get tx from mempool.space: {}", err))?;
        let tx = Tx::consensus_decode(&mut Cursor::new(bytes))
            .map_err(|err| format!("Failed to get tx from mempool.space: {}", err))?;
        Ok(tx)
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_mempool_client_mainnet_tx() {
        let client = super::MemPoolClient::new("https://mempool.space/api");
        let txid = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
            .parse()
            .unwrap();
        let status = client.tx_status(&txid).unwrap();
        assert_eq!(status.block_height, Some(0));
        assert_eq!(status.block_time, Some(1231006505));
    }

    #[test]
    fn test_mempool_client_testnet_tx() {
        let client = super::MemPoolClient::new("https://mempool.space/testnet/api");
        let txid = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
            .parse()
            .unwrap();
        let status = client.tx_status(&txid).unwrap();
        assert_eq!(status.block_height, Some(0));
        assert_eq!(status.block_time, Some(1296688602));
    }

    #[test]
    fn test_mempool_client_testnet4_tx() {
        let client = super::MemPoolClient::new("https://mempool.space/testnet4/api");
        let txid = "7aa0a7ae1e223414cb807e40cd57e667b718e42aaf9306db9102fe28912b7b4e"
            .parse()
            .unwrap();
        let status = client.tx_status(&txid).unwrap();
        assert_eq!(status.block_height, Some(0));
        assert_eq!(status.block_time, Some(1714777860));
    }

    #[test]
    fn test_mempool_client_testnet4_tx_detail() {
        let client = super::MemPoolClient::new("https://mempool.space/testnet4/api");
        let txid = "7aa0a7ae1e223414cb807e40cd57e667b718e42aaf9306db9102fe28912b7b4e"
            .parse()
            .unwrap();
        let tx = client.tx(&txid).expect("Failed to get tx");
        assert!(tx.inputs.len() > 0);
        assert!(tx.outputs.len() > 0);
        assert_eq!(tx.outputs[0].value, 5_000_000_000);
    }
}
