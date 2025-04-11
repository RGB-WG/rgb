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

#![allow(clippy::result_large_err)]

use std::convert::Infallible;
use std::io;

use amplify::IoError;
use bpstd::Psbt;
use nonasync::persistence::PersistenceError;
use psrgbt::{CommitError, ConstructionError, EmbedError, TapretKeyError};
use rgbstd::containers::{LoadError, TransitionInfoError};
use rgbstd::contract::{BuilderError, ContractError};
use rgbstd::persistence::{
    ComposeError, ConsignError, FasciaError, Stock, StockError, StockErrorAll, StockErrorMem,
};
use rgbstd::{AssignmentType, ChainNet};
use strict_types::encoding::Ident;

use crate::validation;

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum WalletError {
    #[from]
    #[from(io::Error)]
    File(IoError),

    #[from]
    StockLoad(LoadError),

    WalletPersist(PersistenceError),

    StockPersist(PersistenceError),

    #[cfg(feature = "cli")]
    #[from]
    WalletExec(bpwallet::cli::ExecError),

    #[from]
    Builder(BuilderError),

    #[from]
    Contract(ContractError),

    Invoicing(String),

    #[from]
    PsbtDecode(psrgbt::DecodeError),

    /// wallet with id '{0}' is not known to the system.
    #[display(doc_comments)]
    WalletUnknown(Ident),

    #[from]
    InvalidConsignment(validation::Status),

    /// invalid identifier.
    #[from]
    #[display(doc_comments)]
    InvalidId(baid64::Baid64ParseError),

    /// the contract source doesn't fit requirements imposed by the used schema.
    ///
    /// {0}
    #[display(doc_comments)]
    IncompleteContract(validation::Status),

    /// cannot find the terminal to add the tapret tweak to.
    NoTweakTerminal,

    /// resolver error: {0}
    #[display(doc_comments)]
    Resolver(String),

    #[from(StockError)]
    #[from(StockErrorAll)]
    #[display(inner)]
    Stock(String),

    #[cfg(feature = "cli")]
    #[from]
    Yaml(serde_yaml::Error),

    #[from]
    Custom(String),
}

impl From<Infallible> for WalletError {
    fn from(_: Infallible) -> Self { unreachable!() }
}

impl From<(Stock, WalletError)> for WalletError {
    fn from((_, e): (Stock, WalletError)) -> Self { e }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Display, Error, From)]
pub enum PayError {
    #[from]
    #[display(inner)]
    Composition(CompositionError),

    #[display("{0}")]
    Completion(CompletionError, Psbt),
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum CompositionError {
    /// invoice doesn't specify a contract.
    NoContract,

    /// invoice doesn't provide information about the assignment type and it's impossible to derive
    /// which assignment type should be used from the schema.
    NoAssignment,

    /// invoice specifies an unknown contract.
    UnknownContract,

    /// state provided via PSBT inputs is not sufficient to cover invoice state
    /// requirements.
    InsufficientState,

    /// the invoice has expired.
    InvoiceExpired,

    /// invoice specifies a schema which is not valid for the specified contract.
    InvalidSchema,

    /// invoice requesting chain-network pair {0} but contract commits to a different one ({1})
    InvoiceBeneficiaryWrongChainNet(ChainNet, ChainNet),

    /// non-fungible state is not yet supported by the invoices.
    Unsupported,

    #[from]
    #[display(inner)]
    Construction(ConstructionError),

    #[from]
    #[display(inner)]
    Contract(ContractError),

    #[from]
    #[display(inner)]
    Embed(EmbedError),

    /// no outputs available to store state of type {0}
    NoExtraOrChange(AssignmentType),

    /// cannot find an output where to put the tapret commitment.
    NoOutputForTapretCommitment,

    /// the provided PSBT doesn't pay any sats to the RGB beneficiary address.
    NoBeneficiaryOutput,

    /// beneficiary output number is given when secret seal is used.
    BeneficiaryVout,

    /// the spent UTXOs contain too many seals which can't fit the state
    /// transition input limit.
    TooManyInputs,

    #[from]
    #[display(inner)]
    Transition(TransitionInfoError),

    /// the operation produces too many extra state transitions which can't fit
    /// the container requirements.
    TooManyExtras,

    #[from]
    #[display(inner)]
    Builder(BuilderError),

    #[from(String)]
    #[from(StockError)]
    #[from(StockErrorMem<ComposeError>)]
    #[display(inner)]
    Stock(String),
}

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum CompletionError {
    /// unspecified contract.
    NoContract,

    /// the provided PSBT doesn't pay any sats to the RGB beneficiary address.
    NoBeneficiaryOutput,

    /// the provided PSBT has conflicting descriptor in the taptweak output.
    InconclusiveDerivation,

    #[from]
    #[display(inner)]
    TapretKey(TapretKeyError),

    #[from]
    #[display(inner)]
    Commit(CommitError),

    #[from(String)]
    #[from(StockErrorMem<ConsignError>)]
    #[from(StockErrorMem<FasciaError>)]
    #[display(inner)]
    Stock(String),
}

impl From<Infallible> for CompletionError {
    fn from(_: Infallible) -> Self { unreachable!() }
}
