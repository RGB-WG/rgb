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

use std::collections::HashMap;
use std::convert::Infallible;
use std::iter;

use bp::seals::txout::CloseMethod;
use bp::Vout;
use psbt::Psbt;
use rgbstd::containers::{Bindle, BuilderSeal, Transfer};
use rgbstd::interface::{BuilderError, ContractSuppl, TypedState, VelocityHint};
use rgbstd::invoice::{Beneficiary, RgbInvoice};
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
        // 2. Construct PSBT
        let beneficiary_output = match invoice.beneficiary {
            Beneficiary::BlindedSeal(seal) => None,
            Beneficiary::WitnessUtxo(addr) => psbt
                .outputs()
                .position(|out| out.script == addr.script_pubkey())
                .ok_or(PayError::NoBeneficiaryOutput)?,
        };
        let prev_outpoints = psbt.inputs().map(|inp| inp.prevout().outpoint());

        // Classify PSBT outputs which can be used for assignments
        let mut out_classes = HashMap::<VelocityHint, Vec<Vout>>::new();
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
        let allocator = |id: ContractId,
                         assignment_type: AssignmentType,
                         velocity: VelocityHint|
         -> Option<Vout> {
            out_classes
                .get_mut(&velocity)
                .and_then(iter::Cycle::next)
                .or_else(|| {
                    out_classes
                        .get_mut(&VelocityHint::default())
                        .and_then(iter::Cycle::next)
                })
        };

        // 4. Add transitions to PSBT
        psbt.rgb_embed(batch)?;

        // 5. Prepare transfer
        let witness_txid = psbt.txid();
        let beneficiary = match beneficiary {
            BuilderSeal::Revealed(seal) => BuilderSeal::Revealed(seal.resolve(witness_txid)),
            BuilderSeal::Concealed(seal) => BuilderSeal::Concealed(seal),
        };
        let transfer = self.stock().transfer(contract_id, [beneficiary])?;

        Ok(transfer)
    }
}
