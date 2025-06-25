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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use amplify::confinement::{Confined, NonEmptyOrdMap, SmallVec, U24};
use amplify::{confinement, FromSliceError, Wrapper};
use bp::dbc::Method;
use bp::seals::txout::CloseMethod;
use bpstd::psbt::{KeyMap, MpcPsbtError, PropKey, Psbt};
use commit_verify::mpc;
use rgbstd::{
    AssignmentType, ContractId, KnownTransition, MergeReveal, MergeRevealError, OpId, Operation,
    Opout, Transition, TransitionBundle,
};
use strict_encoding::{DeserializeError, StrictDeserialize, StrictSerialize};

// TODO: Instead of storing whole RGB contract in PSBT create a shortened
//       contract version which skips all info not important for hardware
//       signers
// /// Proprietary key subtype for storing RGB contract consignment in
// /// global map.
// pub const PSBT_GLOBAL_RGB_CONTRACT: u8 = 0x00;

/// PSBT proprietary key prefix used for RGB.
pub const PSBT_RGB_PREFIX: &str = "RGB";

/// Proprietary key subtype for storing RGB state transition in global map.
pub const PSBT_GLOBAL_RGB_TRANSITION: u64 = 0x01;
/// Proprietary key subtype for storing information on which close method
/// should be used.
pub const PSBT_GLOBAL_RGB_CLOSE_METHOD: u64 = 0x02;
/// Proprietary key subtype to signal that tapret host has been put on change.
pub const PSBT_GLOBAL_RGB_TAP_HOST_CHANGE: u64 = 0x03;
/// Proprietary key subtype for storing RGB input allocation and ID of the
/// transition spending it.
pub const PSBT_GLOBAL_RGB_CONSUMED_BY: u64 = 0x04;

#[derive(Wrapper, WrapperMut, Clone, PartialEq, Eq, Debug, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
pub struct OpoutAndOpids(BTreeMap<Opout, OpId>);

impl OpoutAndOpids {
    pub fn new(items: BTreeMap<Opout, OpId>) -> Self { Self(items) }

    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (opout, opid) in &self.0 {
            bytes.extend(opout.op.to_byte_array());
            bytes.extend(opout.ty.to_le_bytes());
            bytes.extend(opout.no.to_le_bytes());
            bytes.extend(opid.to_byte_array());
        }
        bytes
    }

    #[allow(clippy::result_large_err)]
    pub fn deserialize(bytes: &[u8]) -> Result<Self, RgbPsbtError> {
        let opid_size = std::mem::size_of::<OpId>();
        let assignment_type_size = std::mem::size_of::<u16>();
        let u16_size = std::mem::size_of::<u16>();
        let item_size = opid_size + assignment_type_size + u16_size + opid_size;
        let bytes_len = bytes.len();
        if bytes_len % item_size != 0 {
            return Err(RgbPsbtError::InvalidOpoutAndOpidsData(format!(
                "Input data length {bytes_len} is not a multiple of {item_size}"
            )));
        }
        let mut items = BTreeMap::new();
        for chunk in bytes.chunks_exact(item_size) {
            let mut cursor = 0;
            let op = OpId::copy_from_slice(&chunk[cursor..cursor + opid_size]).map_err(|e| {
                RgbPsbtError::InvalidOpoutAndOpidsData(format!(
                    "Error deserializing Opout.op: {e:?}",
                ))
            })?;
            cursor += opid_size;
            let ty_bytes = &chunk[cursor..cursor + assignment_type_size];
            let ty_u16 = u16::from_le_bytes([ty_bytes[0], ty_bytes[1]]);
            let ty = AssignmentType::with(ty_u16);
            cursor += assignment_type_size;
            let no_bytes = &chunk[cursor..cursor + u16_size];
            let no = u16::from_le_bytes([no_bytes[0], no_bytes[1]]);
            cursor += u16_size;
            let opid = OpId::copy_from_slice(&chunk[cursor..cursor + opid_size]).map_err(|e| {
                RgbPsbtError::InvalidOpoutAndOpidsData(format!(
                    "Error deserializing consuming OpId: {e:?}"
                ))
            })?;
            let opout = Opout::new(op, ty, no);
            items.insert(opout, opid);
        }
        Ok(OpoutAndOpids::new(items))
    }
}

#[allow(clippy::result_large_err)]
fn insert_transitions_sorted(
    transitions: &HashMap<OpId, Transition>,
    known_transitions: &mut SmallVec<KnownTransition>,
) -> Result<(), RgbPsbtError> {
    #[allow(clippy::result_large_err)]
    fn visit_and_insert(
        opid: OpId,
        transitions: &HashMap<OpId, Transition>,
        known_transitions: &mut SmallVec<KnownTransition>,
        visited: &mut HashSet<OpId>,
        visiting: &mut HashSet<OpId>,
    ) -> Result<(), RgbPsbtError> {
        if visited.contains(&opid) {
            return Ok(());
        }
        if visiting.contains(&opid) {
            return Err(RgbPsbtError::KnownTransitionsInconsistency);
        }
        if let Some(transition) = transitions.get(&opid) {
            visiting.insert(opid);
            for input in transition.inputs() {
                if transitions.contains_key(&input.op) {
                    visit_and_insert(input.op, transitions, known_transitions, visited, visiting)?;
                }
            }
            visiting.remove(&opid);
            visited.insert(opid);
            known_transitions
                .push(KnownTransition {
                    opid,
                    transition: transition.clone(),
                })
                .map_err(|_| {
                    RgbPsbtError::InvalidTransitionsNumber(
                        transition.contract_id,
                        transitions.len(),
                    )
                })?;
        }
        Ok(())
    }

    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();
    for &opid in transitions.keys() {
        visit_and_insert(opid, transitions, known_transitions, &mut visited, &mut visiting)?;
    }
    Ok(())
}

/// Extension trait for static functions returning RGB-related proprietary keys.
pub trait ProprietaryKeyRgb {
    /// Constructs [`PSBT_GLOBAL_RGB_TRANSITION`] proprietary key.
    fn rgb_transition(opid: OpId) -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX.to_owned(),
            subtype: PSBT_GLOBAL_RGB_TRANSITION,
            data: opid.to_vec().into(),
        }
    }

    /// Constructs [`PSBT_GLOBAL_RGB_CLOSE_METHOD`] proprietary key.
    fn rgb_close_method() -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX.to_owned(),
            subtype: PSBT_GLOBAL_RGB_CLOSE_METHOD,
            data: none!(),
        }
    }

    /// Constructs [`PSBT_GLOBAL_RGB_CONSUMED_BY`] proprietary key.
    fn rgb_consumed_by(contract_id: ContractId) -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX.to_owned(),
            subtype: PSBT_GLOBAL_RGB_CONSUMED_BY,
            data: contract_id.to_vec().into(),
        }
    }

    /// Constructs [`PSBT_GLOBAL_RGB_TAP_HOST_CHANGE`] proprietary key.
    fn rgb_tapret_host_on_change() -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX.to_owned(),
            subtype: PSBT_GLOBAL_RGB_TAP_HOST_CHANGE,
            data: none!(),
        }
    }
}

impl ProprietaryKeyRgb for PropKey {}

/// Errors processing RGB-related proprietary PSBT keys and their values.
#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum RgbPsbtError {
    /// the key is already present in PSBT, but has a different value
    AlreadySet,

    /// the opout is already signalled as spent by a different opid
    DoubleSpend,

    /// state transition {0} already present in PSBT is not related to the state
    /// transition {1} which has to be added to RGB
    UnrelatedTransitions(OpId, OpId, MergeRevealError),

    /// PSBT contains no contract information
    NoContracts,

    /// PSBT contains no contract consumers information
    NoContractConsumers,

    /// contract {0} listed in the PSBT has an invalid number of known transitions {1}.
    InvalidTransitionsNumber(ContractId, usize),

    /// inputs listed in the PSBT have an invalid number {0}.
    InvalidInputsNumber(usize),

    /// invalid contract id data.
    #[from(FromSliceError)]
    InvalidContractId,

    /// invalid opout and opids data: {0}.
    InvalidOpoutAndOpidsData(String),

    /// Unable to sort bundle's known transitions because of data inconsistency.
    KnownTransitionsInconsistency,

    /// PSBT doesn't provide information about close method.
    NoCloseMethod,

    /// PSBT provides invalid close method information.
    InvalidCloseMethod,

    /// PSBT doesn't specify an output which can host {0} commitment.
    NoHostOutput(Method),

    /// PSBT contains too many contracts (more than 16 million).
    TooManyContracts,

    /// PSBT contains too many state transitions for a bundle.
    #[from(confinement::Error)]
    TooManyTransitionsInBundle,

    /// the size of transition {0} exceeds 16 MB.
    TransitionTooBig(OpId),

    /// state transition data in PSBT are invalid. Details: {0}
    #[from]
    InvalidTransition(DeserializeError),

    #[from]
    #[display(inner)]
    Mpc(MpcPsbtError),
}

#[allow(clippy::result_large_err)]
pub trait RgbExt {
    fn rgb_contract_ids(&self) -> Result<BTreeSet<ContractId>, FromSliceError>;

    fn rgb_contract_consumers(
        &self,
        contract_id: ContractId,
    ) -> Result<BTreeMap<Opout, OpId>, RgbPsbtError>;

    fn rgb_transition(&self, opid: OpId) -> Result<Option<Transition>, RgbPsbtError>;

    fn rgb_close_method(&self) -> Result<Option<CloseMethod>, RgbPsbtError>;

    fn rgb_tapret_host_on_change(&self) -> bool;

    fn set_rgb_close_method(&mut self, close_method: CloseMethod);

    fn set_rgb_tapret_host_on_change(&mut self);

    /// Adds information about an RGB input allocation and the ID of the state
    /// transition spending it.
    ///
    /// # Returns
    ///
    /// `Ok(false)`, if the same opout under the same contract was already
    /// present with the provided state transition ID. `Ok(true)`, if the
    /// opout was successfully added.
    ///
    /// # Errors
    ///
    /// If the [`Opout`] already exists but it's referencing a different [`OpId`].
    fn set_rgb_contract_consumer(
        &mut self,
        contract_id: ContractId,
        opout: Opout,
        opid: OpId,
    ) -> Result<bool, RgbPsbtError>;

    fn push_rgb_transition(&mut self, transition: Transition) -> Result<bool, RgbPsbtError>;

    fn rgb_bundles(&self) -> Result<BTreeMap<ContractId, TransitionBundle>, RgbPsbtError> {
        let mut map = BTreeMap::new();
        for contract_id in self.rgb_contract_ids()? {
            let contract_consumers = self.rgb_contract_consumers(contract_id)?;
            if contract_consumers.is_empty() {
                return Err(RgbPsbtError::NoContractConsumers);
            }
            let inputs_len = contract_consumers.len();
            let input_map = NonEmptyOrdMap::try_from(contract_consumers)
                .map_err(|_| RgbPsbtError::InvalidInputsNumber(inputs_len))?;
            let mut transitions_map: HashMap<OpId, Transition> = HashMap::new();
            for opid in input_map.values() {
                if let Some(transition) = self.rgb_transition(*opid)? {
                    transitions_map.insert(*opid, transition);
                }
            }
            let known_transitions_len = transitions_map.values().len();
            let mut known_transitions: SmallVec<KnownTransition> =
                SmallVec::with_capacity(known_transitions_len);
            insert_transitions_sorted(&transitions_map, &mut known_transitions)?;

            let bundle = TransitionBundle {
                input_map,
                known_transitions: Confined::try_from(known_transitions.release()).map_err(
                    |_| RgbPsbtError::InvalidTransitionsNumber(contract_id, known_transitions_len),
                )?,
            };
            map.insert(contract_id, bundle);
        }
        Ok(map)
    }

    fn rgb_bundles_to_mpc(
        &mut self,
    ) -> Result<Confined<BTreeMap<ContractId, TransitionBundle>, 1, U24>, RgbPsbtError>;
}

impl RgbExt for Psbt {
    fn rgb_contract_ids(&self) -> Result<BTreeSet<ContractId>, FromSliceError> {
        self.proprietary
            .keys()
            .filter(|prop_key| {
                prop_key.identifier == PSBT_RGB_PREFIX
                    && prop_key.subtype == PSBT_GLOBAL_RGB_CONSUMED_BY
            })
            .map(|prop_key| prop_key.data.as_slice())
            .map(ContractId::copy_from_slice)
            .collect()
    }

    fn rgb_contract_consumers(
        &self,
        contract_id: ContractId,
    ) -> Result<BTreeMap<Opout, OpId>, RgbPsbtError> {
        let Some(data) = self.proprietary.get(&PropKey::rgb_consumed_by(contract_id)) else {
            return Ok(BTreeMap::new());
        };
        Ok(OpoutAndOpids::deserialize(data)?.into_inner())
    }

    fn rgb_transition(&self, opid: OpId) -> Result<Option<Transition>, RgbPsbtError> {
        let Some(data) = self.proprietary(&PropKey::rgb_transition(opid)) else {
            return Ok(None);
        };
        let data = Confined::try_from_iter(data.iter().copied())?;
        let transition = Transition::from_strict_serialized::<U24>(data)?;
        Ok(Some(transition))
    }

    fn rgb_close_method(&self) -> Result<Option<CloseMethod>, RgbPsbtError> {
        let Some(m) = self.proprietary(&PropKey::rgb_close_method()) else {
            return Ok(None);
        };
        if m.len() == 1 {
            if let Ok(method) = CloseMethod::try_from(m[0]) {
                return Ok(Some(method));
            }
        }
        Err(RgbPsbtError::InvalidCloseMethod)
    }

    fn rgb_tapret_host_on_change(&self) -> bool {
        self.has_proprietary(&PropKey::rgb_tapret_host_on_change())
    }

    fn set_rgb_close_method(&mut self, close_method: CloseMethod) {
        let _ = self.push_proprietary(PropKey::rgb_close_method(), vec![close_method as u8]);
    }

    fn set_rgb_tapret_host_on_change(&mut self) {
        let _ = self.push_proprietary(PropKey::rgb_tapret_host_on_change(), vec![]);
    }

    fn set_rgb_contract_consumer(
        &mut self,
        contract_id: ContractId,
        opout: Opout,
        opid: OpId,
    ) -> Result<bool, RgbPsbtError> {
        let key = PropKey::rgb_consumed_by(contract_id);
        if let Some(existing_data) = self.proprietary(&key) {
            let mut items = OpoutAndOpids::deserialize(existing_data)?;
            if let Some(existing_opid) = items.get(&opout) {
                if *existing_opid != opid {
                    return Err(RgbPsbtError::DoubleSpend);
                }
                return Ok(false);
            }
            items.insert(opout, opid);
            self.insert_proprietary(key, items.serialize().into());
        } else {
            let items = OpoutAndOpids::new(bmap![opout => opid]);
            let _ = self.push_proprietary(key, items.serialize());
        }
        Ok(true)
    }

    fn push_rgb_transition(&mut self, mut transition: Transition) -> Result<bool, RgbPsbtError> {
        let opid = transition.id();

        let prev_transition = self.rgb_transition(opid)?;
        if let Some(ref prev_transition) = prev_transition {
            transition.merge_reveal(prev_transition).map_err(|err| {
                RgbPsbtError::UnrelatedTransitions(prev_transition.id(), opid, err)
            })?;
        }
        let serialized_transition = transition
            .to_strict_serialized::<U24>()
            .map_err(|_| RgbPsbtError::TransitionTooBig(opid))?;

        // Since we update transition it's ok to ignore the fact that it previously
        // existed
        let _ =
            self.push_proprietary(PropKey::rgb_transition(opid), serialized_transition.release());

        for opout in transition.inputs() {
            self.set_rgb_contract_consumer(transition.contract_id, opout, opid)?;
        }

        Ok(prev_transition.is_none())
    }

    fn rgb_bundles_to_mpc(
        &mut self,
    ) -> Result<Confined<BTreeMap<ContractId, TransitionBundle>, 1, U24>, RgbPsbtError> {
        let bundles = self.rgb_bundles()?;

        let close_method = self
            .rgb_close_method()?
            .ok_or(RgbPsbtError::NoCloseMethod)?;

        let host = self
            .outputs_mut()
            .find(|output| match close_method {
                CloseMethod::OpretFirst => output.is_opret_host(),
                CloseMethod::TapretFirst => output.is_tapret_host(),
            })
            .ok_or(RgbPsbtError::NoHostOutput(close_method))?;

        for (contract_id, bundle) in &bundles {
            let protocol_id = mpc::ProtocolId::from(*contract_id);
            let message = mpc::Message::from(bundle.bundle_id());
            host.set_mpc_message(protocol_id, message)?;
        }

        let map = Confined::try_from(bundles).map_err(|_| RgbPsbtError::NoContracts)?;

        Ok(map)
    }
}
