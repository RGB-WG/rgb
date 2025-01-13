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

use bp::Bp;
use bpwallet::{Layer2, Wallet};
use rgbstd::interface::AssignmentsFilter;

use crate::{DescriptorRgb, XChain, XOutpoint, XWitnessId};

pub struct WalletOutpointsFilter<'a, K, D: DescriptorRgb<K>, L2: Layer2>(pub &'a Wallet<K, D, L2>);

// We need manual derivation to ensure we can be copied and cloned even if descriptor is not
// copyable/clonable.
impl<K, D: DescriptorRgb<K>, L2: Layer2> Copy for WalletOutpointsFilter<'_, K, D, L2> {}
impl<K, D: DescriptorRgb<K>, L2: Layer2> Clone for WalletOutpointsFilter<'_, K, D, L2> {
    fn clone(&self) -> Self { *self }
}

impl<K, D: DescriptorRgb<K>, L2: Layer2> AssignmentsFilter for WalletOutpointsFilter<'_, K, D, L2> {
    fn should_include(&self, output: impl Into<XOutpoint>, _: Option<XWitnessId>) -> bool {
        match output.into().into_bp() {
            Bp::Bitcoin(outpoint) => self.0.has_outpoint(outpoint),
            Bp::Liquid(_) => false,
        }
    }
}

pub struct WalletUnspentFilter<'a, K, D: DescriptorRgb<K>, L2: Layer2>(pub &'a Wallet<K, D, L2>);

// We need manual derivation to ensure we can be copied and cloned even if descriptor is not
// copyable/clonable.
impl<K, D: DescriptorRgb<K>, L2: Layer2> Copy for WalletUnspentFilter<'_, K, D, L2> {}
impl<K, D: DescriptorRgb<K>, L2: Layer2> Clone for WalletUnspentFilter<'_, K, D, L2> {
    fn clone(&self) -> Self { *self }
}

impl<K, D: DescriptorRgb<K>, L2: Layer2> AssignmentsFilter for WalletUnspentFilter<'_, K, D, L2> {
    fn should_include(&self, output: impl Into<XOutpoint>, _: Option<XWitnessId>) -> bool {
        match output.into().into_bp() {
            Bp::Bitcoin(outpoint) => self.0.is_unspent(outpoint),
            Bp::Liquid(_) => false,
        }
    }
}

pub struct WalletWitnessFilter<'a, K, D: DescriptorRgb<K>, L2: Layer2>(pub &'a Wallet<K, D, L2>);

// We need manual derivation to ensure we can be copied and cloned even if descriptor is not
// copyable/clonable.
impl<K, D: DescriptorRgb<K>, L2: Layer2> Copy for WalletWitnessFilter<'_, K, D, L2> {}
impl<K, D: DescriptorRgb<K>, L2: Layer2> Clone for WalletWitnessFilter<'_, K, D, L2> {
    fn clone(&self) -> Self { *self }
}

impl<K, D: DescriptorRgb<K>, L2: Layer2> AssignmentsFilter for WalletWitnessFilter<'_, K, D, L2> {
    fn should_include(&self, _: impl Into<XOutpoint>, witness_id: Option<XWitnessId>) -> bool {
        self.0
            .history()
            .any(|row| !row.our_inputs.is_empty() && witness_id == Some(XChain::Bitcoin(row.txid)))
    }
}
