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

use std::str::FromStr;

use bpstd::Network;
use rgbstd::stl::Precision;
use rgbstd::ContractId;

use super::{Beneficiary, InvoiceState, RgbInvoice, RgbTransport, TransportParseError};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RgbInvoiceBuilder(RgbInvoice);

#[allow(clippy::result_large_err)]
impl RgbInvoiceBuilder {
    pub fn new(beneficiary: impl Into<Beneficiary>) -> Self {
        Self(RgbInvoice {
            transports: vec![RgbTransport::UnspecifiedMeans],
            contract: None,
            iface: None,
            operation: None,
            assignment: None,
            beneficiary: beneficiary.into(),
            owned_state: InvoiceState::Void,
            network: None,
            expiry: None,
            unknown_query: none!(),
        })
    }

    pub fn with(contract_id: ContractId, beneficiary: impl Into<Beneficiary>) -> Self {
        Self::new(beneficiary).set_contract(contract_id)
    }

    pub fn rgb20(contract_id: ContractId, beneficiary: impl Into<Beneficiary>) -> Self {
        Self::with(contract_id, beneficiary).set_interface("RGB20")
    }

    pub fn rgb20_anything(beneficiary: impl Into<Beneficiary>) -> Self {
        Self::new(beneficiary).set_interface("RGB20")
    }

    pub fn set_contract(mut self, contract_id: ContractId) -> Self {
        self.0.contract = Some(contract_id);
        self
    }

    pub fn set_interface(mut self, name: &'static str) -> Self {
        self.0.iface = Some(tn!(name));
        self
    }

    pub fn set_operation(mut self, name: &'static str) -> Self {
        self.0.operation = Some(tn!(name));
        self
    }

    pub fn set_assignment(mut self, name: &'static str) -> Self {
        self.0.assignment = Some(fname!(name));
        self
    }

    pub fn set_amount_raw(mut self, amount: u64) -> Self {
        self.0.owned_state = InvoiceState::Amount(amount);
        self
    }

    pub fn set_amount(
        self,
        integer: u64,
        decimals: u64,
        precision: Precision,
    ) -> Result<Self, Self> {
        // 2^64 ~ 10^19 < 10^18 (18 is max value for Precision enum)
        let pow = 10u64.pow(precision as u32);
        // number of decimals can't be larger than the smallest possible integer
        if decimals >= pow {
            return Err(self);
        }
        let Some(mut amount) = integer.checked_mul(pow) else {
            return Err(self);
        };
        amount = amount.checked_add(decimals).expect(
            "integer has at least the same number of zeros in the lowest digits as much as \
             decimals has digits at most, so overflow is not possible",
        );
        Ok(self.set_amount_raw(amount))
    }

    /// # Safety
    ///
    /// The function may cause the loss of the information about the precise
    /// amout of the asset, since f64 type doesn't provide full precision
    /// required for that.
    pub unsafe fn set_amount_approx(self, amount: f64, precision: Precision) -> Result<Self, Self> {
        if amount <= 0.0 {
            return Err(self);
        }
        let coins = amount.floor();
        let cents = amount - coins;
        self.set_amount(coins as u64, cents as u64, precision)
    }

    pub fn set_network(mut self, network: impl Into<Network>) -> Self {
        self.0.network = Some(network.into());
        self
    }

    pub fn set_expiry_timestamp(mut self, expiry: i64) -> Self {
        self.0.expiry = Some(expiry);
        self
    }

    pub fn add_transport(self, transport: &str) -> Result<Self, (Self, TransportParseError)> {
        let transport = match RgbTransport::from_str(transport) {
            Err(err) => return Err((self, err)),
            Ok(transport) => transport,
        };
        Ok(self.add_transport_raw(transport))
    }

    pub fn add_transport_raw(mut self, transport: RgbTransport) -> Self {
        self.0.transports.push(transport);
        self
    }

    pub fn add_transports<'a>(
        self,
        transports: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, (Self, TransportParseError)> {
        let res = transports
            .into_iter()
            .map(RgbTransport::from_str)
            .collect::<Result<Vec<_>, TransportParseError>>();
        let transports = match res {
            Err(err) => return Err((self, err)),
            Ok(transports) => transports,
        };
        Ok(self.add_transports_raw(transports))
    }

    pub fn add_transports_raw(
        mut self,
        transports: impl IntoIterator<Item = RgbTransport>,
    ) -> Self {
        self.0.transports.extend(transports);
        self
    }

    pub fn finish(self) -> RgbInvoice { self.0 }
}
