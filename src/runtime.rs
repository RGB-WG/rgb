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

use amplify::MultiError;
use bpstd::psbt::{
    Beneficiary, ConstructionError, DbcPsbtError, PsbtConstructor, PsbtMeta, TxParams,
};
use bpstd::seals::TxoSeal;
use bpstd::{Address, Psbt, Sats};
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::{
    BundleError, Coinselect, FulfillError, IncludeError, OpRequestSet, PaymentScript, PrefabBundle,
    RgbWallet, WalletProvider,
};
use rgb::{ContractId, Contracts, EitherSeal, Pile, RgbSealDef, Stock, Stockpile};
use rgpsbt::{RgbPsbt, RgbPsbtCsvError, RgbPsbtPrepareError, ScriptResolver};

use crate::{CoinselectStrategy, Payment};

/// RGB Runtime is a lightweight stateless layer integrating some wallet provider (`Wallet` generic
/// parameter) and RGB stockpile (`Sp` generic parameter).
///
/// It provides
/// - synchronization for the history of witness transactions, extending the main wallet UTXO set
///   synchronization ([`Self::sync`]);
/// - low-level methods for working with PSBTs using `bp-std` library (these methods utilize
///   [`rgb-psbt`] crate) - like [`Self::compose_psbt`] and [`Self::color_psbt`];
/// - high-level payment methods ([`Self::pay`], [`Self::rbf`]) relaying on the above.
// TODO: Support Sp generics
pub struct RgbRuntime<Wallet, Sp>(RgbWallet<Wallet, Sp>)
where
    Wallet: WalletProvider,
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>;

impl<Wallet, Sp> From<RgbWallet<Wallet, Sp>> for RgbRuntime<Wallet, Sp>
where
    Wallet: WalletProvider,
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    fn from(wallet: RgbWallet<Wallet, Sp>) -> Self { Self(wallet) }
}

impl<Wallet, Sp> Deref for RgbRuntime<Wallet, Sp>
where
    Wallet: WalletProvider,
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    type Target = RgbWallet<Wallet, Sp>;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl<Wallet, Sp> DerefMut for RgbRuntime<Wallet, Sp>
where
    Wallet: WalletProvider,
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<Wallet, Sp> RgbRuntime<Wallet, Sp>
where
    Wallet: WalletProvider,
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    pub fn with_components(wallet: Wallet, contracts: Contracts<Sp>) -> Self {
        Self(RgbWallet::with(wallet, contracts))
    }
    pub fn into_rgb_wallet(self) -> RgbWallet<Wallet, Sp> { self.0 }
    pub fn into_components(self) -> (Wallet, Contracts<Sp>) { self.0.unbind() }
}

impl<Wallet, Sp> RgbRuntime<Wallet, Sp>
where
    Wallet: PsbtConstructor + WalletProvider,
    Sp: Stockpile,
    Sp::Pile: Pile<Seal = TxoSeal>,
{
    /// Pay an invoice producing PSBT ready to be signed.
    ///
    /// Should not be used in multi-party protocols like coinjoins, when a PSBT may need to be
    /// modified in the number of inputs or outputs. Use the `construct_psbt` method for such
    /// scenarios.
    ///
    /// If you need more flexibility in constructing payments (do multiple payments with multiple
    /// contracts, use global state etc.) in a single PSBT, please use `pay_custom` APIs and
    /// [`PrefabBundleSet`] instead of this simplified API.
    #[allow(clippy::type_complexity)]
    pub fn pay_invoice(
        &mut self,
        invoice: &RgbInvoice<ContractId>,
        strategy: impl Coinselect,
        params: TxParams,
        giveaway: Option<Sats>,
    ) -> Result<(Psbt, Payment), MultiError<PayError, <Sp::Stock as Stock>::Error>> {
        let request = self
            .fulfill(invoice, strategy, giveaway)
            .map_err(MultiError::from_a)?;
        let script = OpRequestSet::with(request.clone());
        let (psbt, mut payment) = self
            .transfer(script, params)
            .map_err(MultiError::from_other_a)?;
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

    pub fn rbf(&mut self, payment: &Payment, fee: impl Into<Sats>) -> Result<Psbt, PayError> {
        let mut psbt = payment.uncomit_psbt.clone();
        let change = payment
            .psbt_meta
            .change
            .expect("Can't RBF when no change is present");
        let old_fee = psbt.fee().expect("Invalid PSBT with zero inputs");
        let out = psbt
            .output_mut(change.vout.into_usize())
            .expect("invalid PSBT meta-information in the payment");
        out.amount -= fee.into() - old_fee;

        Ok(self.complete(psbt, &payment.bundle)?)
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
    #[allow(clippy::type_complexity)]
    pub fn transfer(
        &mut self,
        script: PaymentScript,
        params: TxParams,
    ) -> Result<(Psbt, Payment), MultiError<TransferError, <Sp::Stock as Stock>::Error>> {
        let payment = self.exec(script, params)?;
        let psbt = self
            .complete(payment.uncomit_psbt.clone(), &payment.bundle)
            .map_err(MultiError::A)?;
        Ok((psbt, payment))
    }

    pub fn compose_psbt(
        &mut self,
        bundle: &PaymentScript,
        params: TxParams,
    ) -> Result<(Psbt, PsbtMeta), ConstructionError> {
        let closes = bundle
            .iter()
            .flat_map(|params| &params.using)
            .map(|used| used.outpoint);

        let network = self.wallet.network();
        let beneficiaries = bundle
            .iter()
            .flat_map(|params| &params.owned)
            .filter_map(|assignment| match &assignment.state.seal {
                EitherSeal::Alt(seal) => seal.as_ref(),
                EitherSeal::Token(_) => None,
            })
            .map(|seal| {
                let address = Address::with(&seal.wout.script_pubkey(), network)
                    .expect("script pubkey which is not representable as an address");
                Beneficiary::new(address, seal.sats)
            });
        self.wallet.construct_psbt(closes, beneficiaries, params)
    }

    /// Fill in RGB information into a pre-composed PSBT, aligning it with the provided payment
    /// script.
    ///
    /// This procedure internally calls [`RgbWallet::bundle`], ensuring all other RGB data (from
    /// other contracts) which were assigned to the UTXOs spent by this RGB, are not lost and
    /// re-assigned to the change output(s) of the PSBT.
    pub fn color_psbt(
        &mut self,
        mut psbt: Psbt,
        mut meta: PsbtMeta,
        script: PaymentScript,
    ) -> Result<Payment, MultiError<TransferError, <Sp::Stock as Stock>::Error>> {
        // From this moment the transaction becomes unmodifiable
        let mut change_vout = meta.change.map(|c| c.vout);
        let request = psbt
            .rgb_resolve(script, &mut change_vout)
            .map_err(MultiError::from_a)?;
        if let Some(c) = meta.change.as_mut() {
            if let Some(vout) = change_vout {
                c.vout = vout
            }
        }

        let bundle = self
            .bundle(request, meta.change.map(|c| c.vout))
            .map_err(MultiError::from_other_a)?;

        psbt.rgb_fill_csv(&bundle).map_err(MultiError::from_a)?;

        Ok(Payment {
            uncomit_psbt: psbt,
            psbt_meta: meta,
            bundle,
            terminals: none!(),
        })
    }

    /// Execute payment script creating PSBT and prefabricated operation bundle.
    ///
    /// The returned PSBT contains only anonymous client-side validation information and is
    /// not modifiable, since it contains RGB data.
    pub fn exec(
        &mut self,
        script: PaymentScript,
        params: TxParams,
    ) -> Result<Payment, MultiError<TransferError, <Sp::Stock as Stock>::Error>> {
        let (psbt, meta) = self
            .compose_psbt(&script, params)
            .map_err(MultiError::from_a)?;
        self.color_psbt(psbt, meta, script)
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

    use rgb_persist_fs::StockpileDir;

    use super::*;

    pub type RgbpRuntimeDir<Wallet> = RgbRuntime<Wallet, StockpileDir<TxoSeal>>;

    pub trait ConsignmentStream {
        fn write(self, writer: impl io::Write) -> io::Result<()>;
    }

    pub struct Transfer<C: ConsignmentStream> {
        pub psbt: Psbt,
        pub consignment: C,
    }
}
