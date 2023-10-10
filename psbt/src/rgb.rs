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

// TODO: Implement state transition ops for PSBT

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use amplify::confinement::{Confined, U24};
use amplify::{confinement, Wrapper};
use bitcoin::psbt::raw::ProprietaryKey;
use bitcoin::psbt::{self, PartiallySignedTransaction as Psbt};
use commit_verify::mpc;
use rgb::{BundleItem, ContractId, OpId, Operation, Transition, TransitionBundle};
use rgbstd::accessors::{MergeReveal, MergeRevealError};
use rgbstd::interface::VelocityHint;
use strict_encoding::{SerializeError, StrictDeserialize, StrictSerialize};

use super::lnpbp4::OutputLnpbp4;
use super::opret::OutputOpret;
use super::tapret::OutputTapret;

// TODO: Instead of storing whole RGB contract in PSBT create a shortened
//       contract version which skips all info not important for hardware
//       signers
// /// Proprietary key subtype for storing RGB contract consignment in
// /// global map.
// pub const PSBT_GLOBAL_RGB_CONTRACT: u8 = 0x00;

/// PSBT proprietary key prefix used for RGB.
pub const PSBT_RGB_PREFIX: &[u8] = b"RGB";

/// Proprietary key subtype for storing RGB state transition in global map.
pub const PSBT_GLOBAL_RGB_TRANSITION: u8 = 0x01;
/// Proprietary key subtype for storing RGB state transition operation id which
/// consumes this input.
pub const PSBT_IN_RGB_CONSUMED_BY: u8 = 0x03;
/// Proprietary key subtype for storing hint for the velocity of the state
/// which can be assigned to the provided output.
pub const PSBT_OUT_RGB_VELOCITY_HINT: u8 = 0x10;

/// Extension trait for static functions returning RGB-related proprietary keys.
pub trait ProprietaryKeyRgb {
    /// Constructs [`PSBT_GLOBAL_RGB_TRANSITION`] proprietary key.
    fn rgb_transition(opid: OpId) -> ProprietaryKey {
        ProprietaryKey {
            prefix: PSBT_RGB_PREFIX.to_vec(),
            subtype: PSBT_GLOBAL_RGB_TRANSITION,
            key: opid.to_vec(),
        }
    }

    /// Constructs [`PSBT_IN_RGB_CONSUMED_BY`] proprietary key.
    fn rgb_in_consumed_by(contract_id: ContractId) -> ProprietaryKey {
        ProprietaryKey {
            prefix: PSBT_RGB_PREFIX.to_vec(),
            subtype: PSBT_IN_RGB_CONSUMED_BY,
            key: contract_id.to_vec(),
        }
    }

    fn rgb_out_velocity_hint() -> ProprietaryKey {
        ProprietaryKey {
            prefix: PSBT_RGB_PREFIX.to_vec(),
            subtype: PSBT_OUT_RGB_VELOCITY_HINT,
            key: vec![],
        }
    }
}

impl ProprietaryKeyRgb for ProprietaryKey {}

/// Errors processing RGB-related proprietary PSBT keys and their values.
#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum RgbPsbtError {
    /// the key is already present in PSBT, but has a different value
    AlreadySet,

    /// state transition {0} already present in PSBT is not related to the state
    /// transition {1} which has to be added to RGB
    UnrelatedTransitions(OpId, OpId, MergeRevealError),

    /// PSBT doesn't specify an output which can host tapret or opret
    /// commitment.
    NoHostOutput,

    /// PSBT contains too many state transitions for a bundle.
    #[from(confinement::Error)]
    TooManyTransitionsInBundle,

    /// state transition data in PSBT are invalid. Details: {0}
    #[from]
    InvalidTransition(SerializeError),
}

#[allow(clippy::result_large_err)]
pub trait RgbExt {
    fn rgb_contract_ids(&self) -> BTreeSet<ContractId>;

    fn rgb_contract_consumers(
        &self,
        contract_id: ContractId,
    ) -> Result<BTreeSet<(OpId, u16)>, RgbPsbtError>;

    fn rgb_op_ids(&self, contract_id: ContractId) -> BTreeSet<OpId>;

    fn rgb_transitions(&self, contract_id: ContractId) -> BTreeMap<OpId, Transition> {
        self.rgb_op_ids(contract_id)
            .into_iter()
            .filter_map(|opid| self.rgb_transition(opid).map(|ts| (opid, ts)))
            .collect()
    }

    fn rgb_transition(&self, opid: OpId) -> Option<Transition>;

    fn push_rgb_transition(&mut self, transition: Transition) -> Result<bool, RgbPsbtError>;

    fn rgb_bundles(&self) -> Result<BTreeMap<ContractId, TransitionBundle>, RgbPsbtError> {
        let mut map = BTreeMap::new();
        for contract_id in self.rgb_contract_ids() {
            let mut items = BTreeMap::new();
            for (opid, no) in self.rgb_contract_consumers(contract_id)? {
                let transition = self.rgb_transition(opid);
                match items.entry(opid) {
                    Entry::Vacant(entry) => {
                        entry.insert(BundleItem {
                            inputs: tiny_bset!(no),
                            transition,
                        });
                    }
                    Entry::Occupied(entry) => {
                        let item = entry.into_mut();
                        if item.transition.is_none() {
                            item.transition = transition;
                        }
                        item.inputs.push(no)?;
                    }
                }
            }
            let bundle = Confined::try_from(items).map(TransitionBundle::from_inner)?;
            map.insert(contract_id, bundle);
        }
        Ok(map)
    }

    fn rgb_bundle_to_lnpbp4(&mut self) -> Result<usize, RgbPsbtError>;
}

impl RgbExt for Psbt {
    fn rgb_contract_ids(&self) -> BTreeSet<ContractId> {
        self.inputs
            .iter()
            .flat_map(|input| {
                input
                    .proprietary
                    .keys()
                    .filter(|prop_key| {
                        prop_key.prefix == PSBT_RGB_PREFIX &&
                            prop_key.subtype == PSBT_IN_RGB_CONSUMED_BY
                    })
                    .map(|prop_key| &prop_key.key)
                    .filter_map(ContractId::from_slice)
            })
            .collect()
    }

    fn rgb_contract_consumers(
        &self,
        contract_id: ContractId,
    ) -> Result<BTreeSet<(OpId, u16)>, RgbPsbtError> {
        let mut consumers: BTreeSet<(OpId, u16)> = bset! {};
        for (no, input) in self.inputs.iter().enumerate() {
            if let Some(opid) = input.rgb_consumer(contract_id) {
                consumers.insert((opid, no as u16));
            }
        }
        Ok(consumers)
    }

    fn rgb_op_ids(&self, contract_id: ContractId) -> BTreeSet<OpId> {
        self.inputs
            .iter()
            .filter_map(|input| input.rgb_consumer(contract_id))
            .collect()
    }

    fn rgb_transition(&self, opid: OpId) -> Option<Transition> {
        let data = self
            .proprietary
            .get(&ProprietaryKey::rgb_transition(opid))?;
        let data = Confined::try_from_iter(data.iter().copied()).ok()?;
        Transition::from_strict_serialized::<U24>(data).ok()
    }

    fn push_rgb_transition(&mut self, mut transition: Transition) -> Result<bool, RgbPsbtError> {
        let opid = transition.id();
        let prev_transition = self.rgb_transition(opid);
        if let Some(ref prev_transition) = prev_transition {
            transition = transition
                .merge_reveal(prev_transition.clone())
                .map_err(|err| {
                    RgbPsbtError::UnrelatedTransitions(prev_transition.id(), opid, err)
                })?;
        }
        let serialized_transition = transition.to_strict_serialized::<U24>()?;
        self.proprietary
            .insert(ProprietaryKey::rgb_transition(opid), serialized_transition.into_inner());
        Ok(prev_transition.is_none())
    }

    fn rgb_bundle_to_lnpbp4(&mut self) -> Result<usize, RgbPsbtError> {
        let bundles = self.rgb_bundles()?;

        let output = self
            .outputs
            .iter_mut()
            .find(|output| output.is_tapret_host() | output.is_opret_host())
            .ok_or(RgbPsbtError::NoHostOutput)?;

        let len = bundles.len();
        for (contract_id, bundle) in bundles {
            output
                .set_lnpbp4_message(mpc::ProtocolId::from(contract_id), bundle.bundle_id().into())
                .map_err(|_| RgbPsbtError::AlreadySet)?;
        }

        Ok(len)
    }
}

pub trait RgbInExt {
    /// Returns information which state transition consumes this PSBT input.
    ///
    /// We do not error on invalid data in order to support future update of
    /// this proprietary key to a standard one. In this case, the invalid
    /// data will be filtered at the moment of PSBT deserialization and this
    /// function will return `None` only in situations when the key is absent.
    fn rgb_consumer(&self, contract_id: ContractId) -> Option<OpId>;

    /// Adds information about state transition consuming this PSBT input.
    ///
    /// # Returns
    ///
    /// `Ok(false)`, if the same node id under the same contract was already
    /// present in the input. `Ok(true)`, if the id node was successfully
    /// added to the input.
    ///
    /// # Errors
    ///
    /// If the input already contains [`PSBT_IN_RGB_NODE_ID`] key with the given
    /// `contract_id` but referencing different [`OpId`].
    #[allow(clippy::result_large_err)]
    fn set_rgb_consumer(
        &mut self,
        contract_id: ContractId,
        opid: OpId,
    ) -> Result<bool, RgbPsbtError>;
}

impl RgbInExt for psbt::Input {
    fn rgb_consumer(&self, contract_id: ContractId) -> Option<OpId> {
        let data = self
            .proprietary
            .get(&ProprietaryKey::rgb_in_consumed_by(contract_id))?;
        OpId::from_slice(data)
    }

    fn set_rgb_consumer(
        &mut self,
        contract_id: ContractId,
        opid: OpId,
    ) -> Result<bool, RgbPsbtError> {
        match self.rgb_consumer(contract_id) {
            None => {
                self.proprietary
                    .insert(ProprietaryKey::rgb_in_consumed_by(contract_id), opid.to_vec());
                Ok(true)
            }
            Some(id) if id == opid => Ok(false),
            Some(_) => Err(RgbPsbtError::AlreadySet),
        }
    }
}

pub trait RgbOutExt {
    /// Returns hint for the velocity of the state which may be assigned to the
    /// provided output.
    ///
    /// We do not error on invalid data in order to support future update of
    /// this proprietary key to a standard one. In this case, the invalid
    /// data will be filtered at the moment of PSBT deserialization and this
    /// function will return `None` only in situations when the key is absent.
    fn rgb_velocity_hint(&self) -> Option<VelocityHint>;

    /// Adds hint for the velocity of the state which may be assigned to the
    /// PSBT output.
    ///
    /// # Returns
    ///
    /// `false`, if a velocity hint was already present in the input and
    /// `true` otherwise.
    fn set_rgb_velocity_hint(&mut self, hint: VelocityHint) -> bool;
}

impl RgbOutExt for psbt::Output {
    fn rgb_velocity_hint(&self) -> Option<VelocityHint> {
        let data = self
            .proprietary
            .get(&ProprietaryKey::rgb_out_velocity_hint())?;
        if data.len() != 1 {
            None
        } else {
            data.first().map(VelocityHint::with_value)
        }
    }

    fn set_rgb_velocity_hint(&mut self, hint: VelocityHint) -> bool {
        let prev = self.rgb_velocity_hint();
        self.proprietary
            .insert(ProprietaryKey::rgb_out_velocity_hint(), vec![hint as u8]);
        Some(hint) == prev
    }
}
