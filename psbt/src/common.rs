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

use bp::{ScriptPubkey, Vout};
use rgb::popls::bp::PrefabBundle;
use rgb::{ContractId, Outpoint};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Display, Error)]
#[display(
    "in order to complete RGB processing the PSBT must have PSBT_GLOBAL_TX_MODIFIABLE flag set on"
)]
pub struct RgbPsbtUnfinalizable;

pub trait RgbPsbt {
    // TODO: Add rgb_embed to embed operations for hardware signers
    fn rgb_fill_csv(&mut self, bundle: &PrefabBundle) -> Result<(), RgbPsbtError>;

    fn rgb_complete(&mut self) -> Result<(), RgbPsbtUnfinalizable>;
}

/// Errors embedding RGB-related information.
#[derive(Clone, Eq, PartialEq, Debug, Display, Error)]
#[display(doc_comments)]
pub enum RgbPsbtError {
    /// input spending {0} which used by RGB operation is absent from PSBT.
    InputAbsent(Outpoint),

    /// input {0} is already used for {1}
    InputAlreadyUsed(usize, ContractId),
}

pub trait ScriptResolver {
    fn script_resolver(&self) -> impl Fn(&ScriptPubkey) -> Option<Vout>;
}
