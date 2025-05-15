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

use amplify::confinement::TinyString;
use chrono::{DateTime, Utc};
use rgb::{Articles, Codex, CodexId, Consensus, ContractId, ContractName, Identity};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractInfo {
    pub id: ContractId,
    pub name: ContractName,
    pub issuer: Identity,
    pub timestamp: DateTime<Utc>,
    pub codex: CodexInfo,
    pub consensus: Consensus,
    pub testnet: bool,
}

impl ContractInfo {
    pub fn new(id: ContractId, articles: &Articles) -> Self {
        Self {
            id,
            name: articles.issue.meta.name.clone(),
            issuer: articles.issue.meta.issuer.clone(),
            timestamp: DateTime::from_timestamp(articles.issue.meta.timestamp, 0)
                .expect("Invalid timestamp"),
            codex: CodexInfo::new(&articles.schema.codex),
            consensus: articles.issue.meta.consensus,
            testnet: articles.issue.meta.testnet,
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexInfo {
    pub id: CodexId,
    pub name: TinyString,
    pub developer: Identity,
    pub timestamp: DateTime<Utc>,
}

impl CodexInfo {
    pub fn new(codex: &Codex) -> Self {
        Self {
            id: codex.codex_id(),
            name: codex.name.clone(),
            developer: codex.developer.clone(),
            timestamp: DateTime::from_timestamp(codex.timestamp, 0).expect("Invalid timestamp"),
        }
    }
}
