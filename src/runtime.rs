// Wallet Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
// Copyright (C) 2025 RGB Consortium, Switzerland.
// Copyright (C) 2019-2025 Dr Maxim Orlovsky.
// All rights under the above copyrights are reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

use std::ops::{Deref, DerefMut};

use bpstd::psbt::{Beneficiary, ConstructionError, PsbtConstructor, PsbtMeta, TxParams};
use bpstd::seals::TxoSeal;
use bpstd::{Address, Psbt};
use rgb::popls::bp::{Barrow, PrefabParamsSet, WoutAssignment};
use rgb::{EitherSeal, Excavate, Pile, Supply};

use crate::wallet::RgbWallet;

pub struct RgbRuntime<S: Supply, P: Pile<Seal = TxoSeal>, X: Excavate<S, P>>(
    Barrow<RgbWallet, S, P, X>,
);

impl<S: Supply, P: Pile<Seal = TxoSeal>, X: Excavate<S, P>> From<Barrow<RgbWallet, S, P, X>>
    for RgbRuntime<S, P, X>
{
    fn from(barrow: Barrow<RgbWallet, S, P, X>) -> Self { Self(barrow) }
}

impl<S: Supply, P: Pile<Seal = TxoSeal>, X: Excavate<S, P>> Deref for RgbRuntime<S, P, X> {
    type Target = Barrow<RgbWallet, S, P, X>;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<S: Supply, P: Pile<Seal = TxoSeal>, X: Excavate<S, P>> DerefMut for RgbRuntime<S, P, X> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<S: Supply, P: Pile<Seal = TxoSeal>, X: Excavate<S, P>> RgbRuntime<S, P, X> {
    pub fn construct_psbt(
        &mut self,
        bundle: &PrefabParamsSet<WoutAssignment>,
        params: TxParams,
    ) -> Result<(Psbt, PsbtMeta), ConstructionError> {
        let closes = bundle
            .iter()
            .flat_map(|params| &params.using)
            .map(|used| used.outpoint);
        let network = self.0.wallet.network();
        let beneficiaries = bundle
            .iter()
            .flat_map(|params| &params.owned)
            .filter_map(|assignment| match &assignment.state.seal {
                EitherSeal::Alt(seal) => Some(seal),
                EitherSeal::Token(_) => None,
            })
            .map(|seal| {
                let address = Address::with(&seal.wout.script_pubkey(), network)
                    .expect("script pubkey which is not representable as an address");
                Beneficiary::new(address, seal.amount)
            });
        self.0.wallet.construct_psbt(closes, beneficiaries, params)
    }
}

#[cfg(feature = "fs")]
pub mod file {
    use rgb::{DirExcavator, FilePile, FileSupply};

    use super::*;

    pub type RgbDirRuntime = RgbRuntime<FileSupply, FilePile<TxoSeal>, DirExcavator<TxoSeal>>;
}
