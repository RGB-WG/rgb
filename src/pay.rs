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

use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;

use bp::dbc::tapret::TapretProof;
use bp::seals::txout::{CloseMethod, ExplicitSeal};
use bp::{Outpoint, Sats, ScriptPubkey, Tx, Vout};
use bpstd::{psbt, Address};
use bpwallet::{Layer2, Layer2Tx, NoLayer2, TxRow, Wallet, WalletDescr};
use psrgbt::{
    Beneficiary as BpBeneficiary, Psbt, PsbtConstructor, PsbtMeta, RgbExt, RgbPsbt, TapretKeyError,
    TxParams,
};
use rgbstd::containers::{AnchorSet, Transfer};
use rgbstd::interface::AssignmentsFilter;
use rgbstd::invoice::{Amount, Beneficiary, InvoiceState, RgbInvoice};
use rgbstd::persistence::{IndexProvider, StashProvider, StateProvider, Stock};
use rgbstd::validation::ResolveWitness;
use rgbstd::{ChainNet, ContractId, DataState};

use crate::invoice::NonFungible;
use crate::validation::WitnessResolverError;
use crate::vm::WitnessOrd;
use crate::{
    CompletionError, CompositionError, DescriptorRgb, PayError, RgbKeychain, Txid,
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

    // TODO: Add method `color` to add RGB information to an already existing PSBT

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
        let contract_id = invoice.contract.ok_or(CompositionError::NoContract)?;

        let close_method = self.descriptor().close_method();

        let iface_name = invoice.iface.clone().ok_or(CompositionError::NoIface)?;
        let iface = stock.iface(iface_name.clone()).map_err(|e| e.to_string())?;
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

        let filter = ContractOutpointsFilter {
            contract_id,
            stock,
            wallet: self,
            _key_phantom: PhantomData,
            _layer2_phantom: PhantomData,
        };
        let contract = stock
            .contract_iface(contract_id, iface_name)
            .map_err(|e| e.to_string())?;
        let prev_outputs = match invoice.owned_state {
            InvoiceState::Amount(amount) => {
                let state: BTreeMap<_, Vec<Amount>> = contract
                    .fungible(assignment_name, &filter)?
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
                        if sum >= amount {
                            false
                        } else {
                            sum += *val;
                            true
                        }
                    })
                    .map(|(_, seal, _)| *seal)
                    .collect::<BTreeSet<_>>();
                if sum < amount {
                    bset![]
                } else {
                    selection
                }
            }
            InvoiceState::Data(NonFungible::RGB21(allocation)) => {
                let data_state = DataState::from(allocation);
                contract
                    .data(assignment_name, &filter)?
                    .filter(|x| x.state == data_state)
                    .map(|x| x.seal)
                    .collect::<BTreeSet<_>>()
            }
            _ => return Err(CompositionError::Unsupported),
        };
        let beneficiaries = match invoice.beneficiary.into_inner() {
            Beneficiary::BlindedSeal(_) => vec![],
            Beneficiary::WitnessVout(pay2vout) => {
                vec![BpBeneficiary::new(
                    Address::new(*pay2vout, invoice.address_network()),
                    params.min_amount,
                )]
            }
        };
        if prev_outputs.is_empty() {
            return Err(CompositionError::InsufficientState);
        }
        let prev_outpoints = prev_outputs.iter().map(|o| Outpoint::new(o.txid, o.vout));
        params.tx.change_keychain = RgbKeychain::for_method(close_method).into();
        let (mut psbt, mut meta) =
            self.construct_psbt(prev_outpoints, &beneficiaries, params.tx)?;

        let beneficiary_script =
            if let Beneficiary::WitnessVout(pay2vout) = invoice.beneficiary.into_inner() {
                Some(pay2vout.script_pubkey())
            } else {
                None
            };
        psbt.outputs_mut()
            .find(|o| o.script.is_p2tr() && Some(&o.script) != beneficiary_script.as_ref())
            .map(|o| o.set_tapret_host().expect("just created"));
        // TODO: Add descriptor id to the tapret host data

        let change_script = meta
            .change_vout
            .and_then(|vout| psbt.output(vout.to_usize()))
            .map(|output| output.script.clone());
        if close_method == CloseMethod::OpretFirst {
            let output = psbt.construct_output_expect(ScriptPubkey::op_return(&[]), Sats::ZERO);
            output.set_opret_host().expect("just created");
            psbt.sort_outputs_by(|output| !output.is_opret_host())
                .expect("PSBT must be modifiable at this stage");
        } else {
            psbt.sort_outputs_by(|output| !output.is_tapret_host())
                .expect("PSBT must be modifiable at this stage");
        };
        if let Some(ref change_script) = change_script {
            for output in psbt.outputs() {
                if output.script == *change_script {
                    meta.change_vout = Some(output.vout());
                    break;
                }
            }
        }

        let beneficiary_vout = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout) => {
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
        let batch = stock
            .compose(invoice, prev_outputs, beneficiary_vout, |_, _, _| meta.change_vout)
            .map_err(|e| e.to_string())?;

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

        let fascia = psbt.rgb_commit()?;
        if matches!(fascia.anchor, AnchorSet::Tapret(_)) {
            let output = psbt
                .dbc_output::<TapretProof>()
                .ok_or(TapretKeyError::NotTaprootOutput)?;
            let terminal = output
                .terminal_derivation()
                .ok_or(CompletionError::InconclusiveDerivation)?;
            let tapret_commitment = output.tapret_commitment()?;
            self.with_descriptor_mut(|descr| {
                descr.with_descriptor_mut(|d| d.add_tapret_tweak(terminal, tapret_commitment))
            })?;
        }

        let witness_id = psbt.txid();
        let (beneficiary1, beneficiary2) = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout) => {
                let s = (*pay2vout).script_pubkey();
                let vout = psbt
                    .outputs()
                    .position(|output| output.script == s)
                    .ok_or(CompletionError::NoBeneficiaryOutput)?;
                let vout = Vout::from_u32(vout as u32);
                let seal = ExplicitSeal::new(Outpoint::new(witness_id, vout));
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
            .transfer(contract_id, beneficiary2, beneficiary1, None)
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
}
