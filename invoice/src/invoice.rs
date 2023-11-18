// RGB wallet library for smart contracts on Bitcoin & Lightning network
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

use bpstd::{Address, Network};
use indexmap::IndexMap;
use rgbstd::{AttachId, ContractId, SecretSeal};
use strict_encoding::{FieldName, TypeName};

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum RgbTransport {
    JsonRpc { tls: bool, host: String },
    RestHttp { tls: bool, host: String },
    WebSockets { tls: bool, host: String },
    Storm {/* todo */},
    UnspecifiedMeans,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display)]
pub enum InvoiceState {
    #[display("")]
    Void,
    #[display("{0}")]
    Amount(u64),
    #[display("...")] // TODO
    Data(Vec<u8> /* StrictVal */),
    #[display(inner)]
    Attach(AttachId),
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Display, From)]
#[display(inner)]
pub enum Beneficiary {
    #[from]
    BlindedSeal(SecretSeal),
    #[from]
    WitnessUtxo(Address),
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RgbInvoice {
    pub transports: Vec<RgbTransport>,
    pub contract: Option<ContractId>,
    pub iface: Option<TypeName>,
    pub operation: Option<TypeName>,
    pub assignment: Option<FieldName>,
    pub beneficiary: Beneficiary,
    pub owned_state: InvoiceState,
    pub network: Option<Network>,
    /// UTC unix timestamp
    pub expiry: Option<i64>,
    pub unknown_query: IndexMap<String, String>,
}
