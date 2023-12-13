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
use bp::{Sats, Vout};
use bpwallet::{Invoice, PsbtMeta, TxParams};
use psbt::Psbt;
use rgbstd::containers::{Bindle, BuilderSeal, Transfer};
use rgbstd::interface::{BuilderError, ContractSuppl, FilterIncludeAll, TypedState, VelocityHint};
use rgbstd::invoice::{Beneficiary, InvoiceState, RgbInvoice};
use rgbstd::persistence::{ConsignerError, Inventory, InventoryError, Stash};
use rgbstd::{
    AssignmentType, ContractId, GraphSeal, Operation, Opout, SealDefinition,
    RGB_NATIVE_DERIVATION_INDEX, RGB_TAPRET_DERIVATION_INDEX,
};

use crate::Runtime;

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum PayError {
    /// unspecified contract.
    NoContract,

    /// unspecified interface.
    NoIface,

    /// invoice doesn't provide information about the operation, and the used
    /// interface do not define default operation.
    NoOperation,

    /// invoice doesn't provide information about the assignment type, and the
    /// used interface do not define default assignment type.
    NoAssignment,

    /// state provided via PSBT inputs is not sufficient to cover invoice state
    /// requirements.
    InsufficientState,

    /// the invoice has expired.
    InvoiceExpired,

    /// non-fungible state is not yet supported by the invoices.
    Unsupported,

    #[from]
    #[display(inner)]
    Inventory(InventoryError<Infallible>),

    #[from]
    #[display(inner)]
    Builder(BuilderError),

    #[from]
    #[display(inner)]
    Consigner(ConsignerError<Infallible, Infallible>),
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TransferParams {
    pub tx: TxParams,
    pub min_amount: Sats,
}

impl Runtime {
    pub fn pay(
        &self,
        invoice: &RgbInvoice,
        method: CloseMethod,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta, Bindle<Transfer>), PayError> {
        let (mut psbt, meta) = self.construct_psbt(invoice, method, params)?;
        // ... here we pass PSBT around signers, if necessary
        let transfer = self.transfer(invoice, &mut psbt)?;
        Ok((psbt, meta, transfer))
    }

    pub fn construct_psbt(
        &self,
        invoice: &RgbInvoice,
        method: CloseMethod,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), PayError> {
        let contract_id = invoice.contract.ok_or(PayError::NoContract)?;

        let iface_name = invoice.iface.ok_or(PayError::NoIface)?;
        let iface = self.stock().iface_by_name(&iface_name)?;
        let contract = self.contract_iface_named(contract_id, iface_name)?;
        let operation = invoice
            .operation
            .or_else(|| iface.default_operation)
            .ok_or(PayError::NoOperation)?;
        let assignment_name = invoice
            .assignment
            .or_else(|| {
                iface
                    .transitions
                    .get(&operation)
                    .and_then(|t| t.default_assignment)
            })
            .ok_or(PayError::NoAssignment)?;
        let outputs = match invoice.owned_state {
            InvoiceState::Amount(amount) => {
                let mut state = contract.fungible(assignment_name, &FilterIncludeAll)?;
                state.sort_by_key(|a| a.value);
                let mut sum = 0u64;
                state
                    .iter()
                    .rev()
                    .take_while(|a| {
                        if sum >= amount {
                            false
                        } else {
                            sum += amount;
                            true
                        }
                    })
                    .map(|a| a.owner)
                    .collect::<Vec<_>>()
            }
            _ => return Err(PayError::Unsupported),
        };
        let inv = match invoice.beneficiary {
            Beneficiary::BlindedSeal(_) => Invoice::with_max(self.wallet().next_address()),
            Beneficiary::WitnessVoutBitcoin(addr) => Invoice::new(addr, params.min_amount),
        };
        let (mut psbt, meta) = self.wallet().construct_psbt(&outputs, inv, params.tx)?;

        let batch =
            self.compose(&invoice, outputs, method, meta.change_vout, |_, _, _| meta.change_vout)?;
        psbt.rgb_embed(batch)?;
        Ok((psbt, meta))
    }

    pub fn transfer(
        &self,
        invoice: &RgbInvoice,
        psbt: &mut Psbt,
    ) -> Result<Bindle<Transfer>, PayError> {
        let contract_id = invoice.contract.ok_or(PayError::NoContract)?;

        psbt.dbc_finalize()?;
        let fascia = psbt.rgb_extract()?;

        let witness_txid = psbt.txid();
        self.stock().consume(fascia)?;
        let beneficiary = match invoice.beneficiary {
            BuilderSeal::Revealed(seal) => BuilderSeal::Revealed(seal.resolve(witness_txid)),
            BuilderSeal::Concealed(seal) => BuilderSeal::Concealed(seal),
        };
        let transfer = self.stock().transfer(contract_id, [beneficiary])?;

        Ok(transfer)
    }
}
