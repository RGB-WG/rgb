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

use std::collections::{BTreeMap, BTreeSet};

use amplify::confinement::{Confined, SmallOrdMap, U24};
use amplify::{confinement, FromSliceError};
use bp::dbc::Method;
use bp::seals::txout::CloseMethod;
use bpstd::psbt;
use bpstd::psbt::{KeyAlreadyPresent, KeyMap, MpcPsbtError, PropKey, Psbt};
use commit_verify::mpc;
use rgbstd::containers::VelocityHint;
use rgbstd::{
    ContractId, InputMap, MergeReveal, MergeRevealError, OpId, Operation, Transition,
    TransitionBundle, Vin,
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
/// Proprietary key subtype for storing RGB state transition operation id which
/// consumes this input.
pub const PSBT_IN_RGB_CONSUMED_BY: u64 = 0x01;
/// Proprietary key subtype for storing hint for the velocity of the state
/// which can be assigned to the provided output.
pub const PSBT_OUT_RGB_VELOCITY_HINT: u64 = 0x01;

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

    /// Constructs [`PSBT_IN_RGB_CONSUMED_BY`] proprietary key.
    fn rgb_in_consumed_by(contract_id: ContractId) -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX.to_owned(),
            subtype: PSBT_IN_RGB_CONSUMED_BY,
            data: contract_id.to_vec().into(),
        }
    }

    /// Constructs [`PSBT_OUT_RGB_VELOCITY_HINT`] proprietary key.
    fn rgb_out_velocity_hint() -> PropKey {
        PropKey {
            identifier: PSBT_RGB_PREFIX.to_owned(),
            subtype: PSBT_OUT_RGB_VELOCITY_HINT,
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

    /// state transition {0} already present in PSBT is not related to the state
    /// transition {1} which has to be added to RGB
    UnrelatedTransitions(OpId, OpId, MergeRevealError),

    /// PSBT contains no contract information
    NoContracts,

    /// PSBT contains no contract consumers information
    NoContractConsumers,

    /// contract {0} listed in the PSBT has zero known transition information.
    NoTransitions(ContractId),

    /// invalid contract id data.
    #[from(FromSliceError)]
    InvalidContractId,

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
    ) -> Result<BTreeSet<(OpId, Vin)>, FromSliceError>;

    fn rgb_op_ids(&self, contract_id: ContractId) -> Result<BTreeSet<OpId>, FromSliceError>;

    fn rgb_transition(&self, opid: OpId) -> Result<Option<Transition>, RgbPsbtError>;

    fn rgb_close_method(&self) -> Result<Option<CloseMethod>, RgbPsbtError>;

    fn set_rgb_close_method(&mut self, close_method: CloseMethod);

    fn push_rgb_transition(&mut self, transition: Transition) -> Result<bool, RgbPsbtError>;

    fn rgb_bundles(&self) -> Result<BTreeMap<ContractId, TransitionBundle>, RgbPsbtError> {
        let mut map = BTreeMap::new();
        for contract_id in self.rgb_contract_ids()? {
            let mut input_map: SmallOrdMap<Vin, OpId> = SmallOrdMap::new();
            let mut known_transitions: SmallOrdMap<OpId, Transition> = SmallOrdMap::new();
            let contract_consumers = self.rgb_contract_consumers(contract_id)?;
            if contract_consumers.is_empty() {
                return Err(RgbPsbtError::NoContractConsumers);
            }
            for (opid, vin) in contract_consumers {
                let transition = self.rgb_transition(opid)?;
                input_map.insert(vin, opid)?;
                if let Some(transition) = transition {
                    known_transitions.insert(opid, transition)?;
                }
            }
            let bundle = TransitionBundle {
                input_map: InputMap::from(
                    Confined::try_from(input_map.release())
                        .map_err(|_| RgbPsbtError::NoTransitions(contract_id))?,
                ),
                known_transitions: Confined::try_from(known_transitions.release())
                    .map_err(|_| RgbPsbtError::NoTransitions(contract_id))?,
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
        self.inputs()
            .flat_map(|input| {
                input
                    .proprietary
                    .keys()
                    .filter(|prop_key| {
                        prop_key.identifier == PSBT_RGB_PREFIX
                            && prop_key.subtype == PSBT_IN_RGB_CONSUMED_BY
                    })
                    .map(|prop_key| prop_key.data.as_slice())
                    .map(ContractId::copy_from_slice)
            })
            .collect()
    }

    fn rgb_contract_consumers(
        &self,
        contract_id: ContractId,
    ) -> Result<BTreeSet<(OpId, Vin)>, FromSliceError> {
        let mut consumers: BTreeSet<(OpId, Vin)> = bset! {};
        for (no, input) in self.inputs().enumerate() {
            if let Some(opid) = input.rgb_consumer(contract_id)? {
                consumers.insert((opid, Vin::from_u32(no as u32)));
            }
        }
        Ok(consumers)
    }

    fn rgb_op_ids(&self, contract_id: ContractId) -> Result<BTreeSet<OpId>, FromSliceError> {
        self.inputs()
            .filter_map(|input| input.rgb_consumer(contract_id).transpose())
            .collect()
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

    fn set_rgb_close_method(&mut self, close_method: CloseMethod) {
        let _ = self.push_proprietary(PropKey::rgb_close_method(), vec![close_method as u8]);
    }

    fn push_rgb_transition(&mut self, mut transition: Transition) -> Result<bool, RgbPsbtError> {
        let opid = transition.id();

        let prev_transition = self.rgb_transition(opid)?;
        if let Some(ref prev_transition) = prev_transition {
            transition = transition
                .merge_reveal(prev_transition.clone())
                .map_err(|err| {
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

pub trait RgbInExt {
    /// Returns information which state transition consumes this PSBT input.
    ///
    /// We do not error on invalid data in order to support future update of
    /// this proprietary key to a standard one. In this case, the invalid
    /// data will be filtered at the moment of PSBT deserialization and this
    /// function will return `None` only in situations when the key is absent.
    fn rgb_consumer(&self, contract_id: ContractId) -> Result<Option<OpId>, FromSliceError>;

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
    ) -> Result<bool, KeyAlreadyPresent>;
}

impl RgbInExt for psbt::Input {
    fn rgb_consumer(&self, contract_id: ContractId) -> Result<Option<OpId>, FromSliceError> {
        let Some(data) = self
            .proprietary
            .get(&PropKey::rgb_in_consumed_by(contract_id))
        else {
            return Ok(None);
        };
        Ok(Some(OpId::copy_from_slice(data)?))
    }

    fn set_rgb_consumer(
        &mut self,
        contract_id: ContractId,
        opid: OpId,
    ) -> Result<bool, KeyAlreadyPresent> {
        let key = PropKey::rgb_in_consumed_by(contract_id);
        match self.rgb_consumer(contract_id) {
            Ok(None) | Err(_) => {
                let _ = self.push_proprietary(key, opid.to_vec());
                Ok(true)
            }
            Ok(Some(id)) if id == opid => Ok(false),
            Ok(Some(_)) => Err(KeyAlreadyPresent(key)),
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
        let data = self.proprietary.get(&PropKey::rgb_out_velocity_hint())?;
        if data.len() != 1 {
            None
        } else {
            data.first().map(VelocityHint::with_value)
        }
    }

    fn set_rgb_velocity_hint(&mut self, hint: VelocityHint) -> bool {
        let prev = self.rgb_velocity_hint();
        self.push_proprietary(PropKey::rgb_out_velocity_hint(), vec![hint as u8])
            .ok();
        Some(hint) == prev
    }
}
