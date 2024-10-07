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
use bp::seals::txout::ExplicitSeal;
use bp::{Outpoint, Sats, ScriptPubkey, Vout};
use bpstd::{Address, psbt};
use bpwallet::{Wallet, WalletDescr};
use psrgbt::{
    Beneficiary as BpBeneficiary, Psbt, PsbtConstructor, PsbtMeta, RgbPsbt, TapretKeyError,
    TxParams,
};
use rgbstd::containers::Transfer;
use rgbstd::interface::OutpointFilter;
use rgbstd::invoice::{Amount, Beneficiary, InvoiceState, RgbInvoice};
use rgbstd::persistence::{IndexProvider, StashProvider, StateProvider, Stock};
use rgbstd::validation::ResolveWitness;
use rgbstd::{ContractId, DataState, XChain, XOutpoint};

use crate::invoice::NonFungible;
use crate::validation::WitnessResolverError;
use crate::vm::{WitnessOrd, XWitnessTx};
use crate::wrapper::WalletWrapper;
use crate::{
    CompletionError, CompositionError, DescriptorRgb, PayError, RgbKeychain, Txid, XWitnessId,
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
    W: WalletProvider<K> + ?Sized,
    K,
    S: StashProvider,
    H: StateProvider,
    P: IndexProvider,
> where W::Descr: DescriptorRgb<K>
{
    contract_id: ContractId,
    stock: &'stock Stock<S, H, P>,
    wallet: &'wallet W,
    _phantom: PhantomData<K>,
}

impl<
    'stock,
    'wallet,
    W: WalletProvider<K> + ?Sized,
    K,
    S: StashProvider,
    H: StateProvider,
    P: IndexProvider,
> OutpointFilter for ContractOutpointsFilter<'stock, 'wallet, W, K, S, H, P>
where W::Descr: DescriptorRgb<K>
{
    fn include_outpoint(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        if !self.wallet.filter().include_outpoint(output) {
            return false;
        }
        matches!(self.stock.contract_assignments_for(self.contract_id, [output]), Ok(list) if !list.is_empty())
    }
}

pub trait WalletProvider<K>: PsbtConstructor
where Self::Descr: DescriptorRgb<K>
{
    type Filter<'a>: Copy + OutpointFilter
    where Self: 'a;
    fn filter(&self) -> Self::Filter<'_>;
    fn with_descriptor_mut<R>(
        &mut self,
        f: impl FnOnce(&mut WalletDescr<K, Self::Descr>) -> R,
    ) -> R;
    fn outpoints(&self) -> impl Iterator<Item = Outpoint>;
    fn txids(&self) -> impl Iterator<Item = Txid>;

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
        let method = self.descriptor().seal_close_method();

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
            _phantom: PhantomData,
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
                state
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
                    .collect::<BTreeSet<_>>()
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
                    Address::new(pay2vout.address, invoice.address_network()),
                    params.min_amount,
                )]
            }
        };
        let prev_outpoints = prev_outputs
            .iter()
            // TODO: Support liquid
            .map(|o| o.as_reduced_unsafe())
            .map(|o| Outpoint::new(o.txid, o.vout));
        params.tx.change_keychain = RgbKeychain::for_method(method).into();
        let (mut psbt, mut meta) =
            self.construct_psbt(prev_outpoints, &beneficiaries, params.tx)?;

        let beneficiary_script =
            if let Beneficiary::WitnessVout(pay2vout) = invoice.beneficiary.into_inner() {
                Some(pay2vout.address.script_pubkey())
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
        psbt.sort_outputs_by(|output| !output.is_tapret_host())
            .expect("PSBT must be modifiable at this stage");
        if let Some(change_script) = change_script {
            for output in psbt.outputs() {
                if output.script == change_script {
                    meta.change_vout = Some(output.vout());
                    break;
                }
            }
        }

        let beneficiary_vout = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout) => {
                let s = pay2vout.address.script_pubkey();
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
            .compose(invoice, prev_outputs, method, beneficiary_vout, |_, _, _| meta.change_vout)
            .map_err(|e| e.to_string())?;

        let methods = batch.close_method_set();
        if methods.has_opret_first() {
            let output = psbt.construct_output_expect(ScriptPubkey::op_return(&[]), Sats::ZERO);
            output.set_opret_host().expect("just created");
        }

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
        if fascia.anchor.has_tapret() {
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

        let witness_txid = psbt.txid();
        let (beneficiary1, beneficiary2) = match invoice.beneficiary.into_inner() {
            Beneficiary::WitnessVout(pay2vout) => {
                let s = pay2vout.address.script_pubkey();
                let vout = psbt
                    .outputs()
                    .position(|output| output.script == s)
                    .ok_or(CompletionError::NoBeneficiaryOutput)?;
                let vout = Vout::from_u32(vout as u32);
                let seal = XChain::Bitcoin(ExplicitSeal::new(
                    pay2vout.method,
                    Outpoint::new(witness_txid, vout),
                ));
                (None, vec![seal])
            }
            Beneficiary::BlindedSeal(seal) => (Some(XChain::Bitcoin(seal)), vec![]),
        };

        struct FasciaResolver {
            witness_id: XWitnessId,
        }
        impl ResolveWitness for FasciaResolver {
            fn resolve_pub_witness(
                &self,
                _: XWitnessId,
            ) -> Result<XWitnessTx, WitnessResolverError> {
                unreachable!()
            }
            fn resolve_pub_witness_ord(
                &self,
                witness_id: XWitnessId,
            ) -> Result<WitnessOrd, WitnessResolverError> {
                assert_eq!(witness_id, self.witness_id);
                Ok(WitnessOrd::Tentative)
            }
        }

        stock
            .consume_fascia(fascia, FasciaResolver {
                witness_id: XChain::Bitcoin(witness_txid),
            })
            .map_err(|e| e.to_string())?;
        let transfer = stock
            .transfer(contract_id, beneficiary2, beneficiary1)
            .map_err(|e| e.to_string())?;

        Ok(transfer)
    }
}

impl<K, D: DescriptorRgb<K>> WalletProvider<K> for Wallet<K, D> {
    type Filter<'a>
        = WalletWrapper<'a, K, D>
    where Self: 'a;
    fn filter(&self) -> Self::Filter<'_> { WalletWrapper(self) }
    fn with_descriptor_mut<R>(&mut self, f: impl FnOnce(&mut WalletDescr<K, D>) -> R) -> R {
        self.descriptor_mut(f)
    }
    fn outpoints(&self) -> impl Iterator<Item = Outpoint> { self.coins().map(|coin| coin.outpoint) }
    fn txids(&self) -> impl Iterator<Item = Txid> { self.transactions().keys().copied() }
}
