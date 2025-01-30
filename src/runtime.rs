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

use bpstd::psbt::{ConstructionError, DbcPsbtError, TxParams};
use bpstd::seals::{TxoSeal, WTxoSeal};
use bpstd::{Psbt, Sats};
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::{
    Barrow, BundleError, FulfillError, IncludeError, OpRequestSet, PaymentScript, PrefabBundle,
};
use rgb::{AuthToken, ContractId, Excavate, Pile, RgbSealDef, Supply};
use rgpsbt::{RgbPsbt, RgbPsbtCsvError, RgbPsbtFinalizeError, ScriptResolver};

use crate::wallet::RgbWallet;
use crate::CoinselectStrategy;

pub struct RgbRuntime<S: Supply, P: Pile<SealDef = WTxoSeal, SealSrc = TxoSeal>, X: Excavate<S, P>>(
    Barrow<RgbWallet, S, P, X>,
);

impl<S: Supply, P: Pile<SealDef = WTxoSeal, SealSrc = TxoSeal>, X: Excavate<S, P>>
    From<Barrow<RgbWallet, S, P, X>> for RgbRuntime<S, P, X>
{
    fn from(barrow: Barrow<RgbWallet, S, P, X>) -> Self { Self(barrow) }
}

impl<S: Supply, P: Pile<SealDef = WTxoSeal, SealSrc = TxoSeal>, X: Excavate<S, P>> Deref
    for RgbRuntime<S, P, X>
{
    type Target = Barrow<RgbWallet, S, P, X>;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<S: Supply, P: Pile<SealDef = WTxoSeal, SealSrc = TxoSeal>, X: Excavate<S, P>> DerefMut
    for RgbRuntime<S, P, X>
{
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<S: Supply, P: Pile<SealDef = WTxoSeal, SealSrc = TxoSeal>, X: Excavate<S, P>>
    RgbRuntime<S, P, X>
{
    /// Pay an invoice producing PSBT ready to be signed.
    ///
    /// Should not be used in multi-party protocols like coinjoins, when a PSBT may needs to be
    /// modified in the number of inputs or outputs. Use `construct_psbt` method for such
    /// scenarios.
    ///
    /// If you need more flexibility in constructing payments (do multiple payments with multiple
    /// contracts, use global state etc.) in a single PSBT, please use `pay_custom` APIs and
    /// [`PrefabBundleSet`] stead of this simplified API.
    pub fn pay_invoice(
        &mut self,
        invoice: &RgbInvoice<ContractId>,
        strategy: CoinselectStrategy,
        params: TxParams,
        giveaway: Option<Sats>,
    ) -> Result<(Psbt, AuthToken), PayError> {
        let request = self.fulfill(invoice, strategy, giveaway)?;
        let script = OpRequestSet::with(request.clone());
        let psbt = self.transfer(script, params)?;
        let terminal = match invoice.auth {
            RgbBeneficiary::Token(auth) => auth,
            RgbBeneficiary::WitnessOut(wout) => request
                .resolve_seal(wout, psbt.script_resolver())
                .expect("witness out must be present in the PSBT")
                .auth_token(),
        };
        Ok((psbt, terminal))
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
    ) -> Result<Psbt, TransferError> {
        let (psbt, bundle) = self.exec(script, params)?;
        let psbt = self.complete(psbt, &bundle)?;
        Ok(psbt)
    }

    /// Execute payment script creating PSBT and prefabricated operation bundle.
    ///
    /// The returned PSBT contain only anonymous client-side validation information and is
    /// modifiable, thus it can be forwarded to other payjoin participants.
    pub fn exec(
        &mut self,
        script: PaymentScript,
        params: TxParams,
    ) -> Result<(Psbt, PrefabBundle), TransferError> {
        let (mut psbt, meta) = self.0.wallet.compose_psbt(&script, params)?;
        let items = script
            .resolve_seals(psbt.script_resolver(), meta.change_vout)
            .map_err(|_| TransferError::ChangeRequired)?;
        let bundle = self.bundle(items, meta.change_vout)?;

        psbt.rgb_fill_csv(&bundle)?;

        Ok((psbt, bundle))
    }

    /// Completes PSBT and includes prefabricated bundle into the contract stockpile.
    pub fn complete(
        &mut self,
        mut psbt: Psbt,
        bundle: &PrefabBundle,
    ) -> Result<Psbt, TransferError> {
        psbt.rgb_complete()?;
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

#[derive(Clone, Debug, Display, Error, From)]
#[display(inner)]
pub enum PayError {
    #[from]
    Fulfill(FulfillError),
    #[from]
    Transfer(TransferError),
}

#[derive(Clone, Debug, Display, Error, From)]
#[display(inner)]
pub enum TransferError {
    #[from]
    PsbtConstruct(ConstructionError),

    #[from]
    PsbtRgb(RgbPsbtCsvError),

    #[from]
    PsbtDbc(DbcPsbtError),

    #[from]
    PsbtFinalize(RgbPsbtFinalizeError),

    #[from]
    Bundle(BundleError),

    #[from]
    Include(IncludeError),

    #[display("transfer doesn't create BTC change output, which is required for RGB change")]
    ChangeRequired,
}

#[cfg(feature = "fs")]
pub mod file {
    use std::io;

    use rgb::{DirExcavator, FilePile, FileSupply};

    use super::*;

    pub type RgbDirRuntime = RgbRuntime<FileSupply, FilePile<WTxoSeal>, DirExcavator<WTxoSeal>>;

    pub trait ConsignmentStream {
        fn write(self, writer: impl io::Write) -> io::Result<()>;
    }

    pub struct Transfer<C: ConsignmentStream> {
        pub psbt: Psbt,
        pub consignment: C,
    }
}
