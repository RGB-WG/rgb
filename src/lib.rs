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

#[macro_use]
extern crate amplify;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde_crate as serde;

mod descriptor;
#[allow(hidden_glob_reexports)]
mod resolvers;
mod wallet;
pub mod pay;
mod errors;
#[cfg(feature = "fs")]
mod store;

pub use descriptor::{DescriptorRgb, RgbDescr, RgbKeychain, TapTweakAlreadyAssigned, TapretKey};
pub use errors::{CompletionError, CompositionError, HistoryError, PayError, WalletError};
pub use pay::{TransferParams, WalletProvider};
#[cfg(any(feature = "electrum_blocking", feature = "esplora_blocking"))]
pub use resolvers::*;
pub use rgbstd::*;
#[cfg(feature = "fs")]
pub use store::{StoredStock, StoredWallet};
pub use wallet::{WalletStock, WalletWrapper};
