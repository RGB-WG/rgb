// Partially signed bitcoin transaction RGB extensions
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2023 by
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

use std::collections::HashMap;

use bpstd::secp256k1::serde::{Deserialize, Serialize};
use psbt::Psbt;
use rgbstd::{AnchoredBundle, ContractId, Outpoint, Transition};

/// A batch of state transitions under different contracts which are associated
/// with some specific transfer and will be anchored within a single layer 1
/// transaction.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Batch {
    pub transitions: Vec<Transition>,
}

/// Structure exported from a PSBT for merging into the stash. It contains a set
/// of finalized state transitions, packed into bundles, and anchored to a
/// single layer 1 transaction.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Fascia {
    pub bundles: Vec<AnchoredBundle>,
}

pub enum EmbedError {}
pub enum ExtractError {}

pub trait RgbPsbt {
    fn rgb_embed(&mut self, batch: Batch) -> Result<(), EmbedError>;
    fn rgb_extract(&mut self) -> Result<Fascia, ExtractError>;
}

impl RgbPsbt for Psbt {
    fn rgb_embed(&mut self, batch: Batch) -> Result<(), EmbedError> {
        let mut contract_inputs = HashMap::<ContractId, Vec<Outpoint>>::new();

        contract_inputs.entry(id).or_default().push(output);
        for (op_id, transition) in batch {
            for input in self.inputs_mut() {
                let outpoint = input.prevout().outpoint();
                if inputs.contains(&outpoint) {
                    input.set_rgb_consumer(transition.contract_id, op_id)?;
                }
            }
            self.push_rgb_transition(transition)?;
        }
        Ok(())
    }

    fn rgb_extract(&mut self) -> Result<Fascia, ExtractError> {}
}
