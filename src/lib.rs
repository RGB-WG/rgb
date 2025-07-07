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

extern crate alloc;
#[macro_use]
extern crate amplify;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
extern crate core;

pub mod descriptor;
mod bp;
mod coinselect;
mod runtime;
mod info;
mod resolvers;

pub use bp::Owner;
pub use coinselect::CoinselectStrategy;
pub use info::{CodexInfo, ContractInfo};
pub use resolvers::Resolver;
#[cfg(feature = "fs")]
pub use runtime::file::{ConsignmentStream, RgbpRuntimeDir, Transfer};
pub use runtime::{PayError, Payment, RgbRuntime, TransferError};
