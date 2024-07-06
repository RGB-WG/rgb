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

use bpwallet::{Save, Wallet};
use rgbstd::interface::{OutpointFilter, WitnessFilter};

use crate::{AssignmentWitness, DescriptorRgb, WalletProvider, XChain, XOutpoint, XWitnessId};

pub struct WalletWrapper<'a, K, D: DescriptorRgb<K>>(pub &'a Wallet<K, D>)
where Wallet<K, D>: Save;

impl<'a, K, D: DescriptorRgb<K>> Copy for WalletWrapper<'a, K, D> where Wallet<K, D>: Save {}
impl<'a, K, D: DescriptorRgb<K>> Clone for WalletWrapper<'a, K, D>
where Wallet<K, D>: Save
{
    fn clone(&self) -> Self { *self }
}

impl<'a, K, D: DescriptorRgb<K>> OutpointFilter for WalletWrapper<'a, K, D>
where Wallet<K, D>: Save
{
    fn include_outpoint(&self, output: impl Into<XOutpoint>) -> bool {
        let output = output.into();
        self.0
            .outpoints()
            .any(|outpoint| XChain::Bitcoin(outpoint) == *output)
    }
}

impl<'a, K, D: DescriptorRgb<K>> WitnessFilter for WalletWrapper<'a, K, D>
where Wallet<K, D>: Save
{
    fn include_witness(&self, witness: impl Into<AssignmentWitness>) -> bool {
        let witness = witness.into();
        self.0
            .txids()
            .any(|txid| AssignmentWitness::Present(XWitnessId::Bitcoin(txid)) == witness)
    }
}
