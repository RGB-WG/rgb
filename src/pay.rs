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

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::error::Error;
use std::iter;

use bp::seals::txout::CloseMethod;
use bp::{Outpoint, Txid};
use chrono::Utc;
use psbt::Psbt;
use rgbinvoice::{Beneficiary, RgbInvoice};
use rgbstd::containers::{Bindle, BuilderSeal, Transfer};
use rgbstd::interface::{BuilderError, ContractSuppl, TypedState, VelocityHint};
use rgbstd::persistence::{ConsignerError, Inventory, InventoryError, Stash};
use rgbstd::{
    AssignmentType, ContractId, GraphSeal, Operation, Opout, SealDefinition,
    RGB_NATIVE_DERIVATION_INDEX, RGB_TAPRET_DERIVATION_INDEX,
};

use crate::Runtime;

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum PayError {
    /// unspecified contract
    #[display(doc_comments)]
    NoContract,

    /// unspecified interface
    #[display(doc_comments)]
    NoIface,

    /// state provided via PSBT inputs is not sufficient to cover invoice state
    /// requirements.
    #[display(doc_comments)]
    InsufficientState,

    /// the invoice has expired
    #[display(doc_comments)]
    InvoiceExpired,

    #[from]
    Inventory(InventoryError<Infallible>),

    #[from]
    Builder(BuilderError),

    #[from]
    Consigner(ConsignerError<Infallible, Infallible>),
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Payment {
    pub unsigned_psbt: Psbt,
    pub transfer: Bindle<Transfer>,
}

impl Runtime {
    pub fn pay(&mut self, invoice: RgbInvoice, method: CloseMethod) -> Result<Payment, PayError> {
        // 1. Prepare the data
        if let Some(expiry) = invoice.expiry {
            if expiry < Utc::now().timestamp() {
                return Err(PayError::InvoiceExpired);
            }
        }
        let contract_id = invoice.contract.ok_or(PayError::NoContract)?;
        let iface = invoice.iface.ok_or(PayError::NoIface)?;
        let mut main_builder =
            self.transition_builder(contract_id, iface.clone(), invoice.operation)?;

        // 2. Construct PSBT

        let (beneficiary_output, beneficiary) = match invoice.beneficiary {
            Beneficiary::BlindedSeal(seal) => {
                let seal = BuilderSeal::Concealed(seal);
                (None, seal)
            }
            Beneficiary::WitnessUtxo(addr) => {
                let vout = psbt
                    .outputs()
                    .position(|out| out.script == addr.script_pubkey())
                    .ok_or(PayError::NoBeneficiaryOutput)?;
                let seal = BuilderSeal::Revealed(SealDefinition::Bitcoin(GraphSeal::new_vout(
                    method, vout,
                )));
                (Some(vout), seal)
            }
        };
        let prev_outpoints = psbt.inputs().map(|inp| inp.prevout().outpoint());

        // Classify PSBT outputs which can be used for assignments
        let mut out_classes = HashMap::<VelocityHint, Vec<usize>>::new();
        for (no, outp) in psbt.outputs().enumerate() {
            if beneficiary_output == Some(no) {
                continue;
            }
            if outp
                // NB: Here we assume that if output has derivation information it belongs to our wallet.
                .bip32_derivation
                .first()
                .map(|(_, src)| src)
                .or_else(|| outp.tap_bip32_derivation.first().map(|(_, d)| &d.origin))
                .and_then(|orig| orig.derivation().iter().rev().nth(1))
                .copied()
                .map(u32::from)
                .filter(|index| *index == RGB_NATIVE_DERIVATION_INDEX || *index == RGB_TAPRET_DERIVATION_INDEX)
                .is_some()
            {
                let class = outp.rgb_velocity_hint().unwrap_or_default();
                out_classes.entry(class).or_default().push(no);
            }
        }
        let mut out_classes = out_classes
            .into_iter()
            .map(|(class, indexes)| (class, indexes.into_iter().cycle()))
            .collect::<HashMap<_, _>>();
        let mut output_for_assignment = |suppl: Option<&ContractSuppl>,
                                         assignment_type: AssignmentType|
         -> Result<BuilderSeal<GraphSeal>, PayError> {
            let velocity = suppl
                .and_then(|suppl| suppl.owned_state.get(&assignment_type))
                .map(|s| s.velocity)
                .unwrap_or_default();
            let vout = out_classes
                .get_mut(&velocity)
                .and_then(iter::Cycle::next)
                .or_else(|| {
                    out_classes
                        .get_mut(&VelocityHint::default())
                        .and_then(iter::Cycle::next)
                })
                .ok_or(PayError::NoBlankOrChange(velocity, assignment_type))?;
            let seal = GraphSeal::new_vout(method, vout);
            Ok(BuilderSeal::Revealed(SealDefinition::Bitcoin(seal)))
        };

        // 2. Prepare and self-consume transition
        let assignment_name = invoice
            .assignment
            .as_ref()
            .or_else(|| main_builder.default_assignment().ok())
            .ok_or(BuilderError::NoDefaultAssignment)?;
        let assignment_id = main_builder
            .assignments_type(assignment_name)
            .ok_or(BuilderError::InvalidStateField(assignment_name.clone()))?;
        // TODO: select supplement basing on the signer trust level
        let suppl = self
            .contract_suppl(contract_id)
            .and_then(|set| set.first())
            .cloned();
        let mut sum_inputs = 0u64;
        for (opout, state) in self.state_for_outputs(contract_id, prev_outpoints)? {
            main_builder = main_builder.add_input(opout)?;
            if opout.ty != assignment_id {
                let seal = output_for_assignment(suppl.as_ref(), opout.ty)?;
                main_builder = main_builder.add_raw_state(opout.ty, seal, state)?;
            } else if let TypedState::Amount(value, _) = state {
                sum_inputs += value;
            }
        }
        // Add change
        let transition = match invoice.owned_state {
            TypedState::Amount(amt) => {
                match sum_inputs.cmp(&amt) {
                    Ordering::Greater => {
                        let seal = output_for_assignment(suppl.as_ref(), assignment_id)?;
                        let change = TypedState::Amount(sum_inputs - amt);
                        main_builder = main_builder.add_raw_state(assignment_id, seal, change)?;
                    }
                    Ordering::Less => return Err(PayError::InsufficientState),
                    Ordering::Equal => {}
                }
                main_builder
                    .add_raw_state(assignment_id, beneficiary, TypedState::Amount(amt))?
                    .complete_transition(contract_id)?
            }
            _ => {
                todo!("only TypedState::Amount is currently supported")
            }
        };

        // 3. Prepare and self-consume other transitions
        let mut contract_inputs = HashMap::<ContractId, Vec<Outpoint>>::new();
        let mut spent_state = HashMap::<ContractId, BTreeMap<Opout, TypedState>>::new();
        for outpoint in prev_outpoints {
            for id in self.contracts_by_outpoints([outpoint])? {
                contract_inputs.entry(id).or_default().push(outpoint);
                if id == contract_id {
                    continue;
                }
                spent_state
                    .entry(id)
                    .or_default()
                    .extend(self.state_for_outpoints(id, [outpoint])?);
            }
        }
        // Construct blank transitions, self-consume them
        let mut other_transitions = HashMap::with_capacity(spent_state.len());
        for (id, opouts) in spent_state {
            let mut blank_builder = self.blank_builder(id, iface.clone())?;
            // TODO: select supplement basing on the signer trust level
            let suppl = self.contract_suppl(id).and_then(|set| set.first());

            for (opout, state) in opouts {
                let seal = output_for_assignment(suppl, opout.ty)?;
                blank_builder = blank_builder
                    .add_input(opout)?
                    .add_raw_state(opout.ty, seal, state)?;
            }

            other_transitions.insert(id, blank_builder.complete_transition(contract_id)?);
        }

        // 4. Add transitions to PSBT
        other_transitions.insert(contract_id, transition);
        for (id, transition) in other_transitions {
            let inputs = contract_inputs.remove(&id).unwrap_or_default();
            for input in psbt.inputs() {
                let outpoint = input.prevout().outpoint();
                if inputs.contains(&outpoint) {
                    input.set_rgb_consumer(id, transition.id())?;
                }
            }
            psbt.push_rgb_transition(transition)?;
        }
        // Here we assume the provided PSBT is final: its inputs and outputs will not be
        // modified after calling this method.
        let bundles = psbt.rgb_bundles()?;
        // TODO: Make it two-staged, such that PSBT editing will be allowed by other
        //       participants as required for multiparty protocols like coinjoin.
        psbt.rgb_bundle_to_lnpbp4()?;
        let anchor = psbt.dbc_conclude(method)?;
        // TODO: Ensure that with PSBTv2 we remove flag allowing PSBT modification.

        // 4. Prepare transfer
        let witness_txid = psbt.txid();
        self.consume_anchor(anchor)?;
        for (id, bundle) in bundles {
            self.consume_bundle(id, bundle, witness_txid.to_byte_array().into())?;
        }
        let beneficiary = match beneficiary {
            BuilderSeal::Revealed(seal) => {
                BuilderSeal::Revealed(seal.resolve(witness_txid.to_byte_array()))
            }
            BuilderSeal::Concealed(seal) => BuilderSeal::Concealed(seal),
        };
        let transfer = self.stock().transfer(contract_id, [beneficiary])?;

        Ok(transfer)
    }
}
