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

use bpwallet::Wallet;
use rgbstd::interface::AssignmentsFilter;

use crate::{DescriptorRgb, WalletProvider, XChain, XOutpoint, XWitnessId};

pub struct WalletOutpointsFilter<'a, K, D: DescriptorRgb<K>>(pub &'a Wallet<K, D>);

// We need manual derivation to ensure we can be copied and cloned even if descriptor is not
// copyable/clonable.
impl<'a, K, D: DescriptorRgb<K>> Copy for WalletOutpointsFilter<'a, K, D> {}
impl<'a, K, D: DescriptorRgb<K>> Clone for WalletOutpointsFilter<'a, K, D> {
    fn clone(&self) -> Self { *self }
}

impl<'a, K, D: DescriptorRgb<K>> AssignmentsFilter for WalletOutpointsFilter<'a, K, D> {
    fn should_include(&self, output: impl Into<XOutpoint>, _: Option<XWitnessId>) -> bool {
        let output = output.into();
        self.0
            .outpoints()
            .any(|outpoint| XChain::Bitcoin(outpoint) == *output)
    }
}

pub struct WitnessOutpointsFilter<'a, K, D: DescriptorRgb<K>>(pub &'a Wallet<K, D>);

// We need manual derivation to ensure we can be copied and cloned even if descriptor is not
// copyable/clonable.
impl<'a, K, D: DescriptorRgb<K>> Copy for WitnessOutpointsFilter<'a, K, D> {}
impl<'a, K, D: DescriptorRgb<K>> Clone for WitnessOutpointsFilter<'a, K, D> {
    fn clone(&self) -> Self { *self }
}

impl<'a, K, D: DescriptorRgb<K>> AssignmentsFilter for WitnessOutpointsFilter<'a, K, D> {
    fn should_include(&self, _: impl Into<XOutpoint>, witness_id: Option<XWitnessId>) -> bool {
        self.0
            .history()
            .any(|row| witness_id == Some(XChain::Bitcoin(row.txid)))
    }
}
