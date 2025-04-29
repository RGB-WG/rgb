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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::convert::Infallible;
use std::marker::PhantomData;

use amplify::confinement::{Confined, U24};
use bp::dbc::tapret::{TapretCommitment, TapretProof};
use bp::dbc::Proof;
use bp::seals::txout::{CloseMethod, ExplicitSeal};
use bp::secp256k1::rand;
use bp::{Outpoint, Sats, ScriptPubkey, Tx, Vout};
use bpstd::{psbt, Address, IdxBase, NormalIndex, Terminal};
use bpwallet::{Layer2, Layer2Tx, NoLayer2, TxRow, Wallet, WalletDescr};
use chrono::Utc;
use commit_verify::mpc::{Message, ProtocolId};
use psrgbt::{
    Beneficiary as BpBeneficiary, Psbt, PsbtConstructor, PsbtMeta, RgbExt, RgbPsbt, TapretKeyError,
    TxParams,
};
use rgbstd::containers::{Batch, BuilderSeal, IndexedConsignment, Transfer, TransitionInfo};
use rgbstd::contract::{AllocatedState, AssignmentsFilter, BuilderError};
use rgbstd::invoice::{Amount, Beneficiary, InvoiceState, RgbInvoice};
use rgbstd::persistence::{IndexProvider, StashInconsistency, StashProvider, StateProvider, Stock};
use rgbstd::validation::{ConsignmentApi, DbcProof, ResolveWitness};
use rgbstd::{
    AssignmentType, ChainNet, ContractId, GraphSeal, Operation, Opout, OutputSeal, RevealedData,
};

use crate::invoice::NonFungible;
use crate::validation::WitnessResolverError;
use crate::vm::WitnessOrd;
use crate::{
    CompletionError, CompositionError, DescriptorRgb, PayError, RgbKeychain, Txid, WalletError,
    WalletOutpointsFilter, WalletUnspentFilter, WalletWitnessFilter,
};

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

struct ContractOutpointsFilter<
    'stock,
    'wallet,
    W: WalletProvider<K, L2> + ?Sized,
    K,
    S: StashProvider,
    H: StateProvider,
    P: IndexProvider,
    L2: Layer2 = NoLayer2,
> where W::Descr: DescriptorRgb<K>
{
    contract_id: ContractId,
    stock: &'stock Stock<S, H, P>,
    wallet: &'wallet W,
    _key_phantom: PhantomData<K>,
    _layer2_phantom: PhantomData<L2>,
}

impl<
        W: WalletProvider<K, L2> + ?Sized,
        K,
        S: StashProvider,
        H: StateProvider,
        P: IndexProvider,
        L2: Layer2,
    > AssignmentsFilter for ContractOutpointsFilter<'_, '_, W, K, S, H, P, L2>
where W::Descr: DescriptorRgb<K>
{
    fn should_include(&self, output: impl Into<Outpoint>, id: Option<Txid>) -> bool {
        let output = output.into();
        if !self.wallet.filter_unspent().should_include(output, id) {
            return false;
        }
        matches!(self.stock.contract_assignments_for(self.contract_id, [output]), Ok(list) if !list.is_empty())
    }
}

pub trait WalletProvider<K, L2: Layer2>: PsbtConstructor
where Self::Descr: DescriptorRgb<K>
{
    fn filter_outpoints(&self) -> impl AssignmentsFilter + Clone;
    fn filter_unspent(&self) -> impl AssignmentsFilter + Clone;
    fn filter_witnesses(&self) -> impl AssignmentsFilter + Clone;
    fn with_descriptor_mut<R>(
        &mut self,
        f: impl FnOnce(&mut WalletDescr<K, Self::Descr, L2::Descr>) -> R,
    ) -> R;
    fn utxos(&self) -> impl Iterator<Item = Outpoint>;
    fn txos(&self) -> impl Iterator<Item = Outpoint>;
    fn txids(&self) -> impl Iterator<Item = Txid>;
    fn history(&self) -> impl Iterator<Item = TxRow<impl Layer2Tx>> + '_;
    fn add_tapret_tweak(
        &mut self,
        terminal: Terminal,
        tapret_commitment: TapretCommitment,
    ) -> Result<(), Infallible>;
    fn try_add_tapret_tweak(&mut self, transfer: Transfer, txid: &Txid) -> Result<(), WalletError>;

    #[allow(clippy::result_large_err)]
    fn pay<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &mut Stock<S, H, P>,
        invoice: &RgbInvoice,
        params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta, Transfer), PayError> {
        let (mut psbt, meta) = self.construct_psbt_rgb(stock, invoice, params)?;
        // ... here we pass PSBT around signers, if necessary
        let transfer = match self.transfer(stock, invoice, &mut psbt) {
            Ok(transfer) => transfer,
            Err(e) => return Err(PayError::Completion(e, psbt)),
        };
        Ok((psbt, meta, transfer))
    }

    #[allow(clippy::result_large_err)]
    fn construct_psbt_rgb<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &Stock<S, H, P>,
        invoice: &RgbInvoice,
        mut params: TransferParams,
    ) -> Result<(Psbt, PsbtMeta), CompositionError> {
        let close_method = self.descriptor().close_method();

        let contract_id = invoice.contract.ok_or(CompositionError::NoContract)?;
        let contract = stock
            .contract_data(contract_id)
            .map_err(|e| e.to_string())?;

        if let Some(invoice_schema) = invoice.schema {
            if invoice_schema != contract.schema.schema_id() {
                return Err(CompositionError::InvalidSchema);
            }
        }

        let contract_genesis = stock
            .as_stash_provider()
            .genesis(contract_id)
            .map_err(|_| CompositionError::UnknownContract)?;
        let contract_chain_net = contract_genesis.chain_net;
        let invoice_chain_net = invoice.chain_network();
        if contract_chain_net != invoice_chain_net {
            return Err(CompositionError::InvoiceBeneficiaryWrongChainNet(
                invoice_chain_net,
                contract_chain_net,
            ));
        }

        if let Some(expiry) = invoice.expiry {
            if expiry < Utc::now().timestamp() {
                return Err(CompositionError::InvoiceExpired);
            }
        }

        let Some(ref assignment_state) = invoice.assignment_state else {
            return Err(CompositionError::NoAssignmentState);
        };

        let invoice_assignment_type = invoice
            .assignment_name
            .as_ref()
            .map(|n| contract.schema.assignment_type(n.clone()));
        let assignment_type = invoice_assignment_type
            .as_ref()
            .or_else(|| {
                let assignment_types = contract
                    .schema
                    .assignment_types_for_state(assignment_state.clone().into());
                if assignment_types.len() == 1 {
                    Some(assignment_types[0])
                } else {
                    contract
                        .schema
                        .default_assignment
                        .as_ref()
                        .filter(|&assignment| assignment_types.contains(&assignment))
                }
            })
            .ok_or(CompositionError::NoAssignmentType)?;
        let transition_type = contract
            .schema
            .default_transition_for_assignment(assignment_type);

        let filter = ContractOutpointsFilter {
            contract_id,
            stock,
            wallet: self,
            _key_phantom: PhantomData,
            _layer2_phantom: PhantomData,
        };
        let prev_outputs = match assignment_state {
            InvoiceState::Amount(amount) => {
                let state: BTreeMap<_, Vec<Amount>> = contract
                    .fungible_raw(*assignment_type, &filter)?
                    .fold(bmap![], |mut set, a| {
                        set.entry(a.seal).or_default().push(a.state);
                        set
                    });
                let mut state: Vec<_> = state
                    .into_iter()
                    .map(|(seal, vals)| (vals.iter().copied().sum::<Amount>(), seal, vals))
                    .collect();
                state.sort_by_key(|(sum, _, _)| *sum);
                let mut sum = Amount::ZERO;
                let selection = state
                    .iter()
                    .rev()
                    .take_while(|(val, _, _)| {
                        if sum >= *amount {
                            false
                        } else {
                            sum += *val;
                            true
                        }
                    })
                    .map(|(_, seal, _)| *seal)
                    .collect::<BTreeSet<_>>();
                if sum < *amount {
                    bset![]
                } else {
                    selection
                }
            }
            InvoiceState::Data(NonFungible::FractionedToken(allocation)) => {
                let data_state = RevealedData::from(*allocation);
                contract
                    .data_raw(*assignment_type, &filter)?
                    .filter(|x| x.state == data_state)
                    .map(|x| x.seal)
                    .collect::<BTreeSet<_>>()
            }
            InvoiceState::Void => contract
                .rights_raw(*assignment_type, &filter)?
                .map(|x| x.seal)
                .collect::<BTreeSet<_>>(),
        };
        if prev_outputs.is_empty() {
            return Err(CompositionError::InsufficientState);
        }
        let prev_outpoints = prev_outputs.iter().map(|o| Outpoint::new(o.txid, o.vout));
        params.tx.change_keychain = RgbKeychain::for_method(close_method).into();

        let (beneficiaries, beneficiary_script) = match invoice.beneficiary.into_inner() {
            Beneficiary::BlindedSeal(_) => (vec![], None),
            Beneficiary::WitnessVout(pay2vout, _) => (
                vec![BpBeneficiary::new(
                    Address::new(*pay2vout, invoice.address_network()),
                    params.min_amount,
                )],
                Some(pay2vout.script_pubkey()),
            ),
        };

        let (mut psbt, mut meta) =
            self.construct_psbt(prev_outpoints, &beneficiaries, params.tx)?;

        let change_script = meta
            .change_vout
            .and_then(|vout| psbt.output(vout.to_usize()))
            .map(|output| output.script.clone());

        match close_method {
            CloseMethod::TapretFirst => {
                let tap_out_script = if let Some(change_script) = change_script.clone() {
                    psbt.set_rgb_tapret_host_on_change();
                    change_script
                } else {
                    match invoice.beneficiary.into_inner() {
                        Beneficiary::WitnessVout(_, Some(ikey)) => {
                            let beneficiary_script = beneficiary_script.unwrap();
                            psbt.outputs_mut()
                                .find(|o| o.script == beneficiary_script)
                                .unwrap()
                                .tap_internal_key = Some(ikey);
                            beneficiary_script
                        }
                        _ => return Err(CompositionError::NoOutputForTapretCommitment),
                    }
                };
                psbt.outputs_mut()
                    .find(|o| o.script.is_p2tr() && o.script == tap_out_script)
                    .map(|o| o.set_tapret_host().expect("just created"));
                // TODO: Add descriptor id to the tapret host data
                psbt.sort_outputs_by(|output| !output.is_tapret_host())
                    .expect("PSBT must be modifiable at this stage");
            }
            CloseMethod::OpretFirst => {
                let output = psbt.construct_output_expect(ScriptPubkey::op_return(&[]), Sats::ZERO);
                output.set_opret_host().expect("just created");
                psbt.sort_outputs_by(|output| !output.is_opret_host())
                    .expect("PSBT must be modifiable at this stage");
            }
        }

        if let Some(ref change_script) = change_script {
            for output in psbt.outputs() {
                if output.script == *change_script {
                    meta.change_vout = Some(output.vout());
                    break;
                }
            }
        }

        let beneficiary_vout = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout, _) => {
                let s = (*pay2vout).script_pubkey();
                let vout = psbt
                    .outputs()
                    .find(|output| output.script == s)
                    .map(psbt::Output::vout)
                    .expect("PSBT without beneficiary address");
                debug_assert_ne!(Some(vout), meta.change_vout);
                Some(vout)
            }
            Beneficiary::BlindedSeal(_) => None,
        };

        #[allow(clippy::type_complexity)]
        let output_for_assignment =
            |assignment_type: AssignmentType| -> Result<BuilderSeal<GraphSeal>, CompositionError> {
                let vout = meta
                    .change_vout
                    .ok_or(CompositionError::NoExtraOrChange(assignment_type))?;
                let seal = GraphSeal::with_blinded_vout(vout, rand::random());
                Ok(BuilderSeal::Revealed(seal))
            };

        let builder_seal = match (invoice.beneficiary.into_inner(), beneficiary_vout) {
            (Beneficiary::BlindedSeal(seal), None) => BuilderSeal::Concealed(seal),
            (Beneficiary::BlindedSeal(_), Some(_)) => {
                return Err(CompositionError::BeneficiaryVout);
            }
            (Beneficiary::WitnessVout(_, _), Some(vout)) => {
                let seal = GraphSeal::with_blinded_vout(vout, rand::random());
                BuilderSeal::Revealed(seal)
            }
            (Beneficiary::WitnessVout(_, _), None) => {
                return Err(CompositionError::NoBeneficiaryOutput);
            }
        };

        let mut main_builder = stock
            .transition_builder_raw(contract_id, transition_type)
            .map_err(|e| e.to_string())?;

        let prev_outputs = prev_outputs.into_iter().collect::<HashSet<OutputSeal>>();
        let mut main_inputs = Vec::<OutputSeal>::new();
        let mut sum_inputs = Amount::ZERO;
        let mut data_inputs = vec![];
        for (output, list) in stock
            .contract_assignments_for(contract_id, prev_outputs.iter().copied())
            .map_err(|e| e.to_string())?
        {
            main_inputs.push(output);
            for (opout, state) in list {
                main_builder = main_builder.add_input(opout, state.clone())?;
                if opout.ty != *assignment_type {
                    let seal = output_for_assignment(opout.ty)?;
                    main_builder = main_builder.add_owned_state_raw(opout.ty, seal, state)?;
                } else if let AllocatedState::Amount(value) = state {
                    sum_inputs += value.into();
                } else if let AllocatedState::Data(value) = state {
                    data_inputs.push(value);
                }
            }
        }

        // Add payments to beneficiary and change
        match assignment_state.clone() {
            InvoiceState::Amount(amt) => {
                // Pay beneficiary
                if sum_inputs < amt {
                    return Err(CompositionError::InsufficientState);
                }

                if amt > Amount::ZERO {
                    main_builder =
                        main_builder.add_fungible_state_raw(*assignment_type, builder_seal, amt)?;
                }

                // Pay change
                if sum_inputs > amt {
                    let change_seal = output_for_assignment(*assignment_type)?;
                    main_builder = main_builder.add_fungible_state_raw(
                        *assignment_type,
                        change_seal,
                        sum_inputs - amt,
                    )?;
                }
            }
            InvoiceState::Data(data) => match data {
                NonFungible::FractionedToken(allocation) => {
                    let lookup_state = RevealedData::from(allocation);
                    if !data_inputs.into_iter().any(|x| x == lookup_state) {
                        return Err(CompositionError::InsufficientState);
                    }

                    main_builder =
                        main_builder.add_data_raw(*assignment_type, builder_seal, lookup_state)?;
                }
            },
            InvoiceState::Void => {
                main_builder = main_builder.add_rights_raw(*assignment_type, builder_seal)?;
            }
        }

        // 3. Prepare other transitions
        // Enumerate state
        let mut extra_state =
            HashMap::<ContractId, HashMap<OutputSeal, HashMap<Opout, AllocatedState>>>::new();
        for id in stock
            .contracts_assigning(prev_outputs.iter().copied())
            .map_err(|e| e.to_string())?
        {
            // Skip current contract
            if id == contract_id {
                continue;
            }
            let state = stock
                .contract_assignments_for(id, prev_outputs.iter().copied())
                .map_err(|e| e.to_string())?;
            let entry = extra_state.entry(id).or_default();
            for (seal, assigns) in state {
                entry.entry(seal).or_default().extend(assigns);
            }
        }

        // Construct transitions for extra state
        let mut extras = Confined::<Vec<_>, 0, { U24 - 1 }>::with_capacity(extra_state.len());
        for (id, seal_map) in extra_state {
            let schema = stock
                .as_stash_provider()
                .contract_schema(id)
                .map_err(|_| BuilderError::Inconsistency(StashInconsistency::ContractAbsent(id)))?;

            for (output, assigns) in seal_map {
                for (opout, state) in assigns {
                    let transition_type = schema.default_transition_for_assignment(&opout.ty);

                    let mut extra_builder = stock
                        .transition_builder_raw(id, transition_type)
                        .map_err(|e| e.to_string())?;

                    let seal = output_for_assignment(opout.ty)?;
                    extra_builder = extra_builder
                        .add_input(opout, state.clone())?
                        .add_owned_state_raw(opout.ty, seal, state)?;

                    if !extra_builder.has_inputs() {
                        continue;
                    }
                    let transition = extra_builder.complete_transition()?;
                    let info = TransitionInfo::new(transition, [output])
                        .map_err(|_| CompositionError::TooManyInputs)?;
                    extras
                        .push(info)
                        .map_err(|_| CompositionError::TooManyExtras)?;
                }
            }
        }

        if !main_builder.has_inputs() {
            return Err(CompositionError::InsufficientState);
        }

        let main = TransitionInfo::new(main_builder.complete_transition()?, main_inputs)
            .map_err(|_| CompositionError::TooManyInputs)?;
        let mut batch = Batch { main, extras };
        batch.set_priority(u64::MAX);

        psbt.set_rgb_close_method(close_method);
        psbt.complete_construction();
        psbt.rgb_embed(batch)?;
        Ok((psbt, meta))
    }

    #[allow(clippy::result_large_err)]
    fn transfer<S: StashProvider, H: StateProvider, P: IndexProvider>(
        &mut self,
        stock: &mut Stock<S, H, P>,
        invoice: &RgbInvoice,
        psbt: &mut Psbt,
    ) -> Result<Transfer, CompletionError> {
        let contract_id = invoice.contract.ok_or(CompletionError::NoContract)?;

        let beneficiary_vout = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout, _) => {
                let s = (*pay2vout).script_pubkey();
                let vout = psbt
                    .outputs()
                    .position(|output| output.script == s)
                    .ok_or(CompletionError::NoBeneficiaryOutput)?;
                Some(Vout::from_u32(vout as u32))
            }
            Beneficiary::BlindedSeal(_) => None,
        };

        let fascia = psbt.rgb_commit()?;
        if matches!(fascia.seal_witness.dbc_proof.method(), CloseMethod::TapretFirst) {
            // save tweak only if tapret commitment is on the bitcoin change
            if psbt.rgb_tapret_host_on_change() {
                let output = psbt
                    .dbc_output::<TapretProof>()
                    .ok_or(TapretKeyError::NotTaprootOutput)?;
                let terminal = output
                    .terminal_derivation()
                    .ok_or(CompletionError::InconclusiveDerivation)?;
                let tapret_commitment = output.tapret_commitment()?;
                self.add_tapret_tweak(terminal, tapret_commitment)?;
            }
        }

        let witness_id = psbt.txid();
        let (beneficiary1, beneficiary2) = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(_, _) => {
                let seal = ExplicitSeal::new(Outpoint::new(witness_id, beneficiary_vout.unwrap()));
                (None, vec![seal])
            }
            Beneficiary::BlindedSeal(seal) => (Some(seal), vec![]),
        };

        struct FasciaResolver {
            witness_id: Txid,
        }
        impl ResolveWitness for FasciaResolver {
            fn resolve_pub_witness(&self, _: Txid) -> Result<Tx, WitnessResolverError> {
                unreachable!()
            }
            fn resolve_pub_witness_ord(
                &self,
                witness_id: Txid,
            ) -> Result<WitnessOrd, WitnessResolverError> {
                assert_eq!(witness_id, self.witness_id);
                Ok(WitnessOrd::Tentative)
            }
            fn check_chain_net(&self, _: ChainNet) -> Result<(), WitnessResolverError> {
                unreachable!()
            }
        }

        stock
            .consume_fascia(fascia, FasciaResolver { witness_id })
            .map_err(|e| e.to_string())?;
        let transfer = stock
            .transfer(contract_id, beneficiary2, beneficiary1, Some(witness_id))
            .map_err(|e| e.to_string())?;

        Ok(transfer)
    }
}

impl<K, D: DescriptorRgb<K>, L2: Layer2> WalletProvider<K, L2> for Wallet<K, D, L2> {
    fn filter_outpoints(&self) -> impl AssignmentsFilter + Clone { WalletOutpointsFilter(self) }
    fn filter_unspent(&self) -> impl AssignmentsFilter + Clone { WalletUnspentFilter(self) }
    fn filter_witnesses(&self) -> impl AssignmentsFilter + Clone { WalletWitnessFilter(self) }
    fn with_descriptor_mut<R>(
        &mut self,
        f: impl FnOnce(&mut WalletDescr<K, D, L2::Descr>) -> R,
    ) -> R {
        self.descriptor_mut(f)
    }
    fn utxos(&self) -> impl Iterator<Item = Outpoint> { self.coins().map(|coin| coin.outpoint) }
    fn txos(&self) -> impl Iterator<Item = Outpoint> { self.txos().map(|txo| txo.outpoint) }
    fn txids(&self) -> impl Iterator<Item = Txid> { self.transactions().keys().copied() }

    fn history(&self) -> impl Iterator<Item = TxRow<impl Layer2Tx>> + '_ { self.history() }

    fn add_tapret_tweak(
        &mut self,
        terminal: Terminal,
        tapret_commitment: TapretCommitment,
    ) -> Result<(), Infallible> {
        self.with_descriptor_mut(|descr| {
            descr.with_descriptor_mut(|d| {
                d.add_tapret_tweak(terminal, tapret_commitment);
                Ok::<_, Infallible>(())
            })
        })
    }

    fn try_add_tapret_tweak(&mut self, transfer: Transfer, txid: &Txid) -> Result<(), WalletError> {
        let indexed_consignment = IndexedConsignment::new(&transfer);
        let contract_id = transfer.genesis.contract_id();
        let close_method = self.descriptor().close_method();
        let keychain = RgbKeychain::for_method(close_method);
        let last_index = self.next_derivation_index(keychain, false).index() as u16;
        let descr = self.descriptor();
        if let Some((idx, tweak)) = transfer
            .bundles
            .iter()
            .find(|bw| bw.witness_id() == *txid)
            .and_then(|bw| {
                let bundle_id = bw.bundle().bundle_id();
                let (_, anchor) = indexed_consignment.anchor(bundle_id).unwrap();
                if let DbcProof::Tapret(tapret) = anchor.dbc_proof.clone() {
                    let commitment = anchor
                        .mpc_proof
                        .clone()
                        .convolve(ProtocolId::from(contract_id), Message::from(bundle_id))
                        .unwrap();
                    let tweak = TapretCommitment::with(commitment, tapret.path_proof.nonce());
                    (0..last_index)
                        .rev()
                        .map(NormalIndex::normal)
                        .find(|i| {
                            descr
                                .derive(keychain, i)
                                .any(|ds| ds.to_internal_pk() == Some(tapret.internal_pk))
                        })
                        .map(|idx| (idx, tweak))
                } else {
                    None
                }
            })
        {
            self.add_tapret_tweak(Terminal::new(keychain, idx), tweak)
                .unwrap();
            return Ok(());
        }
        Err(WalletError::NoTweakTerminal)
    }
}
