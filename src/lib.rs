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

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
// #![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "async", allow(async_fn_in_trait))]

#[cfg(all(
    feature = "async",
    any(
        feature = "resolver-mempool",
        feature = "resolver-esplora",
        feature = "resolver-electrum",
        feature = "resolver-bitcoinrpc"
    )
))]
compile_error!("async feature must not be used with non-async resolvers");
#[cfg(all(feature = "async", feature = "fs"))]
compile_error!("async feature must not be used with fs feature");

extern crate alloc;
#[macro_use]
extern crate amplify;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
extern crate core;

pub mod descriptor;
mod owner;
mod coinselect;
mod runtime;
mod info;
pub mod resolvers;

pub use coinselect::CoinselectStrategy;
pub use info::{CodexInfo, ContractInfo};
#[cfg(feature = "fs")]
pub use owner::file::FileOwner;
pub use owner::{MemUtxos, Owner, UtxoSet};
#[cfg(feature = "fs")]
pub use runtime::file::{ConsignmentStream, RgbpRuntimeDir, Transfer};
pub use runtime::{FinalizeError, PayError, Payment, RgbRuntime, TransferError};
