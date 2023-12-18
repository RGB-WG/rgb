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

use std::convert::Infallible;

use bp::seals::txout::CloseMethod;
use bp::{Outpoint, Sats, ScriptPubkey, Vout};
use bpwallet::{Beneficiary as BpBeneficiary, ConstructionError, PsbtMeta, TxParams};
use psbt::{CommitError, EmbedError, Psbt, RgbPsbt};
use rgbstd::containers::{Bindle, BuilderSeal, Transfer};
use rgbstd::interface::{ContractError, FilterIncludeAll};
use rgbstd::invoice::{Beneficiary, InvoiceState, RgbInvoice};
use rgbstd::persistence::{
    ComposeError, ConsignerError, Inventory, InventoryError, Stash, StashError,
};
use rgbstd::XSeal;

use crate::{RgbKeychain, Runtime};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum PayError {
    #[from]
    Composition(CompositionError),

    #[from]
    Completion(CompletionError),
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum CompositionError {
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

    /// one of the RGB assignments spent require presence of tapret output -
    /// even this is not a taproot wallet. Unable to create a valid PSBT, manual
    /// work is needed.
    TapretRequired,

    /// non-fungible state is not yet supported by the invoices.
    Unsupported,

    #[from]
    #[display(inner)]
    Construction(ConstructionError),

    #[from]
    #[display(inner)]
    Interface(ContractError),

    #[from]
    #[display(inner)]
    Inventory(InventoryError<Infallible>),

    #[from]
    #[display(inner)]
    Stash(StashError<Infallible>),

    #[from]
    #[display(inner)]
    Compose(ComposeError<Infallible, Infallible>),

    #[from]
    #[display(inner)]
    Embed(EmbedError),
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum CompletionError {
    /// unspecified contract.
    NoContract,

    /// the provided PSBT doesn't pay any sats to the RGB beneficiary address.
    NoBeneficiaryOutput,

    #[from]
    #[display(inner)]
    Inventory(InventoryError<Infallible>),

    #[from]
    #[display(inner)]
    Consigner(ConsignerError<Infallible, Infallible>),

    #[from]
    #[display(inner)]
    Commit(CommitError),
}

#[derive(Clone, PartialEq, Debug)]
pub struct TransferParams {
    pub tx: TxParams,
    pub min_amount: Sats,
}

impl TransferParams {
    pub fn with(fee: Sats, min_amount: Sats) -> Self {
        TransferParams {
            tx: TxParams::with(fee),
            min_amount,
        }
    }
}

impl Runtime {
    #[allow(clippy::result_large_err)]
    pub fn pay(
        &mut self,
        invoice: &RgbInvoice,
        method: CloseMethod,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta, Bindle<Transfer>), PayError> {
        let (mut psbt, meta) = self.construct_psbt(invoice, method, params)?;
        // ... here we pass PSBT around signers, if necessary
        let transfer = self.transfer(invoice, &mut psbt)?;
        Ok((psbt, meta, transfer))
    }

    #[allow(clippy::result_large_err)]
    pub fn construct_psbt(
        &mut self,
        invoice: &RgbInvoice,
        method: CloseMethod,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), CompositionError> {
        let contract_id = invoice.contract.ok_or(CompositionError::NoContract)?;

        let iface_name = invoice.iface.clone().ok_or(CompositionError::NoIface)?;
        let iface = self.stock().iface_by_name(&iface_name)?;
        let contract = self.contract_iface_named(contract_id, iface_name)?;
        let operation = invoice
            .operation
            .as_ref()
            .or(iface.default_operation.as_ref())
            .ok_or(CompositionError::NoOperation)?;
        let assignment_name = invoice
            .assignment
            .as_ref()
            .or_else(|| {
                iface
                    .transitions
                    .get(operation)
                    .and_then(|t| t.default_assignment.as_ref())
            })
            .cloned()
            .ok_or(CompositionError::NoAssignment)?;
        let outputs = match invoice.owned_state {
            InvoiceState::Amount(amount) => {
                let mut state = contract
                    .fungible(assignment_name, &FilterIncludeAll)?
                    .into_inner();
                state.sort_by_key(|a| a.value);
                let mut sum = 0u64;
                state
                    .iter()
                    .rev()
                    .take_while(|a| {
                        if sum >= amount {
                            false
                        } else {
                            sum += a.value;
                            true
                        }
                    })
                    .map(|a| a.owner)
                    .collect::<Vec<_>>()
            }
            _ => return Err(CompositionError::Unsupported),
        };
        let beneficiary = match invoice.beneficiary {
            Beneficiary::BlindedSeal(_) => BpBeneficiary::with_max(
                self.wallet_mut()
                    .next_address(RgbKeychain::for_method(method), true),
            ),
            Beneficiary::WitnessVoutBitcoin(addr) => BpBeneficiary::new(addr, params.min_amount),
        };
        let outpoints = outputs
            .iter()
            .filter_map(|o| o.reduce_to_bp())
            .map(|o| Outpoint::new(o.txid, o.vout));
        let (mut psbt, meta) =
            self.wallet_mut()
                .construct_psbt(outpoints, &[beneficiary], params.tx)?;

        let (beneficiary_vout, beneficiary_script) = match invoice.beneficiary {
            Beneficiary::WitnessVoutBitcoin(addr) => {
                let s = addr.script_pubkey();
                let vout = psbt
                    .outputs()
                    .position(|output| output.script == s)
                    .map(|vout| Vout::from_u32(vout as u32));
                (vout, s)
            }
            Beneficiary::BlindedSeal(_) => (None, none!()),
        };
        let batch =
            self.compose(invoice, outputs, method, beneficiary_vout, |_, _, _| meta.change_vout)?;

        let methods = batch.close_method_set();
        if methods.has_tapret_first() {
            let output = psbt
                .outputs_mut()
                .find(|o| o.script.is_p2tr() && o.script != beneficiary_script)
                .ok_or(CompositionError::TapretRequired)?;
            output.set_tapret_host().expect("just created");
        }
        if methods.has_opret_first() {
            let output = psbt.construct_output_expect(ScriptPubkey::op_return(&[]), Sats::ZERO);
            output.set_opret_host().expect("just created");
        }

        psbt.rgb_embed(batch)?;
        Ok((psbt, meta))
    }

    #[allow(clippy::result_large_err)]
    pub fn transfer(
        &mut self,
        invoice: &RgbInvoice,
        psbt: &mut Psbt,
    ) -> Result<Bindle<Transfer>, CompletionError> {
        let contract_id = invoice.contract.ok_or(CompletionError::NoContract)?;

        let beneficiary = match invoice.beneficiary {
            Beneficiary::WitnessVoutBitcoin(addr) => {
                let s = addr.script_pubkey();
                let vout = psbt
                    .outputs()
                    .position(|output| output.script == s)
                    .ok_or(CompletionError::NoBeneficiaryOutput)?;
                let witness_txid = psbt.txid();
                BuilderSeal::Revealed(XSeal::Bitcoin(
                    Outpoint::new(witness_txid, Vout::from_u32(vout as u32)).into(),
                ))
            }
            Beneficiary::BlindedSeal(seal) => BuilderSeal::Concealed(seal),
        };

        let fascia = psbt.rgb_commit()?;
        self.stock_mut().consume(fascia)?;
        let transfer = self.stock().transfer(contract_id, [beneficiary])?;

        Ok(transfer)
    }
}
