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
use bp::seals::txout::CloseMethod;
use bpstd::Psbt;
use nonasync::persistence::PersistenceError;
use psrgbt::{CommitError, ConstructionError, EmbedError, TapretKeyError};
use rgbstd::containers::LoadError;
use rgbstd::interface::{BuilderError, ContractError};
use rgbstd::persistence::{
    ComposeError, ConsignError, ContractIfaceError, FasciaError, Stock, StockError, StockErrorAll,
    StockErrorMem,
};
use strict_types::encoding::Ident;

use crate::{validation, TapTweakAlreadyAssigned};

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

    /// resolver error: {0}
    #[display(doc_comments)]
    Resolver(String),

    #[from(StockError)]
    #[from(StockErrorAll)]
    #[from(StockErrorMem<ContractIfaceError>)]
    #[display(inner)]
    Stock(String),

    #[cfg(feature = "serde_yaml")]
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
    /// unspecified contract.
    NoContract,

    /// unspecified interface.
    NoIface,

    /// invoice doesn't provide information about the operation, and the used
    /// interface do not define default operation.
    NoOperation,

    /// invoice doesn't provide information about the assignment type, and the
    /// used interface do not define default assignment type.
    NoAssignment,

    /// state provided via PSBT inputs is not sufficient to cover invoice state
    /// requirements.
    InsufficientState,

    /// the invoice has expired.
    InvoiceExpired,

    /// the invoice doesn't support the contract close method {0}
    InvoiceUnsupportsCloseMethod(CloseMethod),

    /// the wallet descriptor doesn't support the contract close method {0}
    WalletUnsupportsCloseMethod(CloseMethod),

    /// one of the RGB assignments spent require presence of tapret output -
    /// even this is not a taproot wallet. Unable to create a valid PSBT, manual
    /// work is needed.
    TapretRequired,

    /// non-fungible state is not yet supported by the invoices.
    Unsupported,

    #[from]
    #[display(inner)]
    Construction(ConstructionError),

    #[from]
    #[display(inner)]
    Interface(ContractError),

    #[from]
    #[display(inner)]
    Embed(EmbedError),

    #[from(String)]
    #[from(StockError)]
    #[from(StockErrorMem<ComposeError>)]
    #[from(StockErrorMem<ContractIfaceError>)]
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

    /// the invoice doesn't support the contract close method {0}
    InvoiceUnsupportsCloseMethod(CloseMethod),

    /// the wallet descriptor doesn't support the contract close method {0}
    WalletUnsupportsCloseMethod(CloseMethod),

    #[from]
    #[display(inner)]
    MultipleTweaks(TapTweakAlreadyAssigned),

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
