// Partially signed bitcoin transaction RGB extensions
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2020-2023 by
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

#[macro_use]
extern crate amplify;

mod rgb;

use bp::dbc::opret::OpretProof;
use bp::dbc::tapret::TapretProof;
pub use psbt::*;
pub use rgb::*;
use rgbstd::containers::{Batch, Fascia};
use rgbstd::{AnchorSet, XAnchor, XChain};

pub use self::rgb::{
    ProprietaryKeyRgb, RgbExt, RgbInExt, RgbOutExt, RgbPsbtError, PSBT_GLOBAL_RGB_TRANSITION,
    PSBT_IN_RGB_CONSUMED_BY, PSBT_OUT_RGB_VELOCITY_HINT, PSBT_RGB_PREFIX,
};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error)]
#[display(doc_comments)]
pub enum EmbedError {
    /// provided transaction batch references inputs which are absent from the
    /// PSBT. Possible it was created for a different PSBT.
    AbsentInputs,

    /// the provided PSBT is invalid since it doublespends on some of its
    /// inputs.
    PsbtRepeatedInputs,
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(inner)]
pub enum CommitError {
    #[from]
    Rgb(RgbPsbtError),

    #[from]
    Dbc(DbcPsbtError),
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error)]
#[display(doc_comments)]
pub enum ExtractError {}

// TODO: Batch must be homomorphic by the outpoint type (chain)

pub trait RgbPsbt {
    fn rgb_embed(&mut self, batch: Batch) -> Result<(), EmbedError>;
    #[allow(clippy::result_large_err)]
    fn rgb_commit(&mut self) -> Result<Fascia, CommitError>;
    fn rgb_extract(&self) -> Result<Fascia, ExtractError>;
}

impl RgbPsbt for Psbt {
    fn rgb_embed(&mut self, batch: Batch) -> Result<(), EmbedError> {
        for info in batch {
            let contract_id = info.transition.contract_id;
            let mut inputs = info.inputs.into_inner();
            for input in self.inputs_mut() {
                let outpoint = input.prevout().outpoint();
                if let Some(pos) = inputs.iter().position(|i| **i == XChain::Bitcoin(outpoint)) {
                    inputs.remove(pos);
                    input
                        .set_rgb_consumer(contract_id, info.id)
                        .map_err(|_| EmbedError::PsbtRepeatedInputs)?;
                }
            }
            if !inputs.is_empty() {
                return Err(EmbedError::AbsentInputs);
            }
            self.push_rgb_transition(info.transition, info.methods)
                .expect("transitions are unique since they are in BTreeMap indexed by opid");
        }
        Ok(())
    }

    fn rgb_commit(&mut self) -> Result<Fascia, CommitError> {
        // Convert RGB data to MPCs? Or should we do it at the moment we add them... No,
        // since we may require more DBC methods with each additional state transition
        let (bundles, methods) = self.rgb_bundles_to_mpc()?;
        // DBC commitment for the required methods
        let (mut tapret_anchor, mut opret_anchor) = (None, None);
        if methods.has_tapret_first() {
            tapret_anchor = Some(self.dbc_commit::<TapretProof>()?);
        }
        if methods.has_opret_first() {
            opret_anchor = Some(self.dbc_commit::<OpretProof>()?);
        }
        let anchor = AnchorSet::from_split(tapret_anchor, opret_anchor)
            .expect("at least one of DBC are present due to CloseMethodSet type guarantees");
        Ok(Fascia {
            anchor: XAnchor::Bitcoin(anchor),
            bundles,
        })
    }

    fn rgb_extract(&self) -> Result<Fascia, ExtractError> {
        todo!("implement RGB PSBT fascia extraction for multi-party protocols")
    }
}
