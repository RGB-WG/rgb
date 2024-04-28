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

use bpwallet::Wallet;
use rgbstd::interface::{AmountChange, IfaceOp, IfaceRef, OutpointFilter, WitnessFilter};
use rgbstd::persistence::{IndexProvider, StashProvider, StateProvider, Stock};

use crate::{
    AssignmentWitness, ContractId, DescriptorRgb, HistoryError, WalletError, WalletProvider,
    XChain, XOutpoint, XWitnessId,
};

pub struct WalletWrapper<'a, K, D: DescriptorRgb<K>>(pub &'a Wallet<K, D>);

impl<'a, K, D: DescriptorRgb<K>> Copy for WalletWrapper<'a, K, D> {}
impl<'a, K, D: DescriptorRgb<K>> Clone for WalletWrapper<'a, K, D> {
    fn clone(&self) -> Self { WalletWrapper(self.0) }
}

impl<'a, K, D: DescriptorRgb<K>> OutpointFilter for WalletWrapper<'a, K, D> {
    fn include_outpoint(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        self.0
            .outpoints()
            .any(|outpoint| XChain::Bitcoin(outpoint) == *output)
    }
}

impl<'a, K, D: DescriptorRgb<K>> WitnessFilter for WalletWrapper<'a, K, D> {
    fn include_witness(&self, witness: impl Into<AssignmentWitness>) -> bool {
        let witness = witness.into();
        self.0
            .txids()
            .any(|txid| AssignmentWitness::Present(XWitnessId::Bitcoin(txid)) == witness)
    }
}

pub trait WalletStock<W: WalletProvider<K>, K>
where W::Descr: DescriptorRgb<K>
{
    fn fungible_history(
        &self,
        wallet: &W,
        contract_id: ContractId,
        iface: impl Into<IfaceRef>,
    ) -> Result<HashMap<XWitnessId, IfaceOp<AmountChange>>, WalletError>;
}

impl<W: WalletProvider<K>, K, S: StashProvider, H: StateProvider, P: IndexProvider>
    WalletStock<W, K> for Stock<S, H, P>
where W::Descr: DescriptorRgb<K>
{
    // TODO: Integrate into BP Wallet `TxRow` as L2 and provide transactional info
    fn fungible_history(
        &self,
        wallet: &W,
        contract_id: ContractId,
        iface: impl Into<IfaceRef>,
    ) -> Result<HashMap<XWitnessId, IfaceOp<AmountChange>>, WalletError> {
        let iref = iface.into();
        let iface = self.iface(iref.clone()).map_err(|e| e.to_string())?;
        let default_op = iface
            .default_operation
            .as_ref()
            .ok_or(HistoryError::NoDefaultOp)?;
        let state_name = iface
            .transitions
            .get(default_op)
            .ok_or(HistoryError::DefaultOpNotTransition)?
            .default_assignment
            .as_ref()
            .ok_or(HistoryError::NoDefaultAssignment)?
            .clone();
        let contract = self
            .contract_iface(contract_id, iref)
            .map_err(|e| e.to_string())?;
        Ok(contract
            .fungible_ops::<AmountChange>(state_name, wallet.filter(), wallet.filter())
            .map_err(|e| e.to_string())?)
    }
}
