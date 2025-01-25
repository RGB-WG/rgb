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

use amplify::ByteArray;
use bpstd::seals::{mmb, mpc};
use bpstd::{Psbt, Sats, ScriptPubkey, Vout};
use rgb::popls::bp::PrefabBundle;

use crate::common::RgbPsbtUnfinalizable;
use crate::{RgbPsbt, RgbPsbtError, ScriptResolver};

impl RgbPsbt for Psbt {
    fn rgb_fill_csv(&mut self, bundle: &PrefabBundle) -> Result<(), RgbPsbtError> {
        for prefab in bundle {
            let id = mpc::ProtocolId::from_byte_array(prefab.operation.contract_id.to_byte_array());
            let opid = prefab.operation.opid();
            let msg = mmb::Message::from_byte_array(opid.to_byte_array());
            for outpoint in &prefab.closes {
                let input = self
                    .inputs_mut()
                    .find(|inp| inp.previous_outpoint == *outpoint)
                    .ok_or(RgbPsbtError::InputAbsent(*outpoint))?;
                input.set_mmb_message(id, msg).map_err(|_| {
                    RgbPsbtError::InputAlreadyUsed(input.index(), prefab.operation.contract_id)
                })?;
            }
        }
        Ok(())
    }

    fn rgb_complete(&mut self) -> Result<(), RgbPsbtUnfinalizable> {
        if self.outputs().all(|out| !out.is_opret_host()) && self.opret_hosts().count() == 0 {
            let host = self
                .construct_output(ScriptPubkey::op_return(&[]), Sats::ZERO)
                .map_err(|_| RgbPsbtUnfinalizable)?;
            host.set_opret_host().ok();
        }
        self.complete_construction();
        Ok(())
    }
}

impl ScriptResolver for Psbt {
    fn script_resolver(&self) -> impl Fn(&ScriptPubkey) -> Option<Vout> {
        |spk| -> Option<Vout> {
            self.outputs()
                .find(|inp| &inp.script == spk)
                .map(|inp| Vout::from_u32(inp.index() as u32))
        }
    }
}
