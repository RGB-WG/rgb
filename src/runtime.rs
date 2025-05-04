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

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ops::{Deref, DerefMut};

use bpstd::psbt::{ConstructionError, DbcPsbtError, TxParams};
use bpstd::seals::TxoSeal;
use bpstd::{Psbt, Sats};
use bpwallet::{Indexer, TxStatus};
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::{
    BundleError, FulfillError, IncludeError, OpRequestSet, PaymentScript, PrefabBundle, RgbWallet,
};
use rgb::{AcceptError, ContractId, Pile, RgbSealDef, Stockpile, WitnessStatus};
use rgpsbt::{RgbPsbt, RgbPsbtCsvError, RgbPsbtPrepareError, ScriptResolver};
use strict_types::SerializeError;

use crate::owner::Owner;
use crate::{CoinselectStrategy, Payment};

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum SyncError<E: Error> {
    #[display("unable to retrieve wallet updates: {_0:?}")]
    Update(Vec<E>),
    Status(E),
    #[from]
    Rollback(SerializeError),
    #[from]
    Forward(AcceptError),
}

pub struct RgbRuntime<Sp>(RgbWallet<Owner, Sp>)
where
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>;

impl<Sp> From<RgbWallet<Owner, Sp>> for RgbRuntime<Sp>
where
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    fn from(barrow: RgbWallet<Owner, Sp>) -> Self { Self(barrow) }
}

impl<Sp> Deref for RgbRuntime<Sp>
where
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    type Target = RgbWallet<Owner, Sp>;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<Sp> DerefMut for RgbRuntime<Sp>
where
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<Sp> RgbRuntime<Sp>
where
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    pub fn sync<I>(&mut self, indexer: &I) -> Result<(), SyncError<I::Error>>
    where
        I: Indexer,
        I::Error: Error,
    {
        if let Some(err) = self.wallet.update(indexer).err {
            return Err(SyncError::Update(err));
        }

        let txids = self.contracts.witness_ids().collect::<HashSet<_>>();
        let mut changed = HashMap::new();
        for txid in txids {
            let status = indexer.status(txid).map_err(SyncError::Status)?;
            let status = match status {
                TxStatus::Unknown => WitnessStatus::Archived,
                TxStatus::Mempool => WitnessStatus::Tentative,
                TxStatus::Channel => WitnessStatus::Offchain,
                TxStatus::Mined(status) => WitnessStatus::Mined(status.height.into()),
            };
            changed.insert(txid, status);
        }
        self.contracts.sync(&changed)?;

        Ok(())
    }

    /// Pay an invoice producing PSBT ready to be signed.
    ///
    /// Should not be used in multi-party protocols like coinjoins, when a PSBT may need to be
    /// modified in the number of inputs or outputs. Use the `construct_psbt` method for such
    /// scenarios.
    ///
    /// If you need more flexibility in constructing payments (do multiple payments with multiple
    /// contracts, use global state etc.) in a single PSBT, please use `pay_custom` APIs and
    /// [`PrefabBundleSet`] instead of this simplified API.
    pub fn pay_invoice(
        &mut self,
        invoice: &RgbInvoice<ContractId>,
        strategy: CoinselectStrategy,
        params: TxParams,
        giveaway: Option<Sats>,
    ) -> Result<(Psbt, Payment), PayError> {
        let request = self.fulfill(invoice, strategy, giveaway)?;
        let script = OpRequestSet::with(request.clone());
        let (psbt, mut payment) = self.transfer(script, params)?;
        let terminal = match invoice.auth {
            RgbBeneficiary::Token(auth) => auth,
            RgbBeneficiary::WitnessOut(wout) => request
                .resolve_seal(wout, psbt.script_resolver())
                .expect("witness out must be present in the PSBT")
                .auth_token(),
        };
        payment.terminals.insert(terminal);
        Ok((psbt, payment))
    }

    pub fn rbf(&mut self, payment: &Payment) -> Result<Psbt, PayError> {
        let psbt = self.complete(payment.uncomit_psbt.clone(), &payment.bundle)?;
        Ok(psbt)
    }

    /// Convert invoice into a payment script.
    pub fn script(
        &mut self,
        invoice: &RgbInvoice<ContractId>,
        strategy: CoinselectStrategy,
        giveaway: Option<Sats>,
    ) -> Result<PaymentScript, PayError> {
        let request = self.fulfill(invoice, strategy, giveaway)?;
        Ok(OpRequestSet::with(request))
    }

    /// Construct transfer, consisting of PSBT and a consignment stream
    // TODO: Return a dedicated Transfer object which can stream a consignment
    pub fn transfer(
        &mut self,
        script: PaymentScript,
        params: TxParams,
    ) -> Result<(Psbt, Payment), TransferError> {
        let payment = self.exec(script, params)?;
        let psbt = self.complete(payment.uncomit_psbt.clone(), &payment.bundle)?;
        Ok((psbt, payment))
    }

    /// Execute payment script creating PSBT and prefabricated operation bundle.
    ///
    /// The returned PSBT contains only anonymous client-side validation information and is
    /// modifiable, thus it can be forwarded to other payjoin participants.
    // TODO: PSBT is not modifiable since it commits to Vouts in the bundle!
    pub fn exec(
        &mut self,
        script: PaymentScript,
        params: TxParams,
    ) -> Result<Payment, TransferError> {
        let (mut psbt, mut meta) = self.0.wallet.compose_psbt(&script, params)?;

        // From this moment transaction becomes unmodifiable
        let request = psbt.rgb_resolve(script, &mut meta.change_vout)?;
        let bundle = self.bundle(request, meta.change_vout)?;

        psbt.rgb_fill_csv(&bundle)?;

        Ok(Payment {
            uncomit_psbt: psbt,
            psbt_meta: meta,
            bundle,
            terminals: none!(),
        })
    }

    /// Completes PSBT and includes the prefabricated bundle into the contract.
    pub fn complete(
        &mut self,
        mut psbt: Psbt,
        bundle: &PrefabBundle,
    ) -> Result<Psbt, TransferError> {
        let (mpc, dbc) = psbt.dbc_commit()?;
        let tx = psbt.to_unsigned_tx();

        let prevouts = psbt
            .inputs()
            .map(|inp| inp.previous_outpoint)
            .collect::<Vec<_>>();
        self.include(bundle, &tx.into(), mpc, dbc, &prevouts)?;

        Ok(psbt)
    }
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum PayError {
    #[from]
    Fulfill(FulfillError),
    #[from]
    Transfer(TransferError),
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum TransferError {
    #[from]
    PsbtConstruct(ConstructionError),

    #[from]
    PsbtRgbCsv(RgbPsbtCsvError),

    #[from]
    PsbtDbc(DbcPsbtError),

    #[from]
    PsbtPrepare(RgbPsbtPrepareError),

    #[from]
    Bundle(BundleError),

    #[from]
    Include(IncludeError),
}

#[cfg(feature = "fs")]
pub mod file {
    use std::io;

    use rgb::StockpileDir;

    use super::*;

    pub type RgbpRuntimeDir = RgbRuntime<StockpileDir<TxoSeal>>;

    pub trait ConsignmentStream {
        fn write(self, writer: impl io::Write) -> io::Result<()>;
    }

    pub struct Transfer<C: ConsignmentStream> {
        pub psbt: Psbt,
        pub consignment: C,
    }
}
