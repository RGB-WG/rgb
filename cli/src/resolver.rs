// RGB smart contract wallet runtime
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

use bpstd::{Tx, Txid};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{ResolveTx, TxResolverError};
use rgbstd::{Layer1, WitnessAnchor, XAnchor};

use crate::RgbArgs;

// TODO: Embed in contract issuance builder
pub struct PanickingResolver;
impl ResolveHeight for PanickingResolver {
    type Error = Infallible;
    fn resolve_anchor(&mut self, _: &XAnchor) -> Result<WitnessAnchor, Self::Error> {
        unreachable!("PanickingResolver must be used only for newly issued contract validation")
    }
}
impl ResolveTx for PanickingResolver {
    fn resolve_bp_tx(&self, _: Layer1, _: Txid) -> Result<Tx, TxResolverError> {
        unreachable!("PanickingResolver must be used only for newly issued contract validation")
    }
}

impl RgbArgs {
    pub fn resolver(&self) -> Result<impl ResolveTx + ResolveHeight, esplora::Error> {
        esplora::Builder::new(&self.resolver.esplora).build_blocking()
    }
}
