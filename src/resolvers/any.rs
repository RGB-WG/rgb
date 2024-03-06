// RGB smart contracts for Bitcoin & Lightning
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2024 by
//     Zoe Faltibà <zoefaltiba@gmail.com>
//
// Copyright (C) 2024 LNP/BP Standards Association. All rights reserved.
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

use rgbstd::containers::Consignment;
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{ResolveWitness, WitnessResolverError};
use rgbstd::{WitnessAnchor, WitnessId, XAnchor, XPubWitness};

#[cfg(feature = "electrum")]
use crate::electrum;
#[cfg(feature = "esplora_blocking")]
use crate::esplora_blocking;

/// Type that contains any of the [`Resolver`] types defined by the library
#[derive(From)]
#[non_exhaustive]
pub enum AnyResolver {
    #[cfg(feature = "electrum")]
    #[from]
    /// Electrum resolver
    Electrum(Box<electrum::Resolver>),
    #[cfg(feature = "esplora_blocking")]
    #[from]
    /// Esplora resolver
    Esplora(Box<esplora_blocking::Resolver>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum AnyResolverError {
    #[cfg(feature = "electrum")]
    #[display(inner)]
    Electrum(::electrum::Error),
    #[cfg(feature = "esplora_blocking")]
    #[display(inner)]
    Esplora(esplora::Error),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum AnyAnchorResolverError {
    #[cfg(feature = "electrum")]
    #[from]
    #[display(inner)]
    Electrum(electrum::AnchorResolverError),
    #[cfg(feature = "esplora_blocking")]
    #[from]
    #[display(inner)]
    Esplora(esplora_blocking::AnchorResolverError),
}

impl AnyResolver {
    pub fn add_terminals<const TYPE: bool>(&mut self, consignment: &Consignment<TYPE>) {
        match self {
            #[cfg(feature = "electrum")]
            AnyResolver::Electrum(inner) => inner.add_terminals(consignment),
            #[cfg(feature = "esplora_blocking")]
            AnyResolver::Esplora(inner) => inner.add_terminals(consignment),
        }
    }
}

impl ResolveHeight for AnyResolver {
    type Error = AnyAnchorResolverError;

    fn resolve_anchor(&mut self, anchor: &XAnchor) -> Result<WitnessAnchor, Self::Error> {
        match self {
            #[cfg(feature = "electrum")]
            AnyResolver::Electrum(inner) => inner.resolve_anchor(anchor).map_err(|e| e.into()),
            #[cfg(feature = "esplora_blocking")]
            AnyResolver::Esplora(inner) => inner.resolve_anchor(anchor).map_err(|e| e.into()),
        }
    }
}

impl ResolveWitness for AnyResolver {
    fn resolve_pub_witness(
        &self,
        witness_id: WitnessId,
    ) -> Result<XPubWitness, WitnessResolverError> {
        match self {
            #[cfg(feature = "electrum")]
            AnyResolver::Electrum(inner) => inner.resolve_pub_witness(witness_id),
            #[cfg(feature = "esplora_blocking")]
            AnyResolver::Esplora(inner) => inner.resolve_pub_witness(witness_id),
        }
    }
}
