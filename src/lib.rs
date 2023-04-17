// RGB smart contracts for Bitcoin & Lightning
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
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

mod descriptor;
mod runtime;
mod wallet;

pub mod prelude {
    pub use super::*;

    pub use descriptor::{RgbDescr, SpkDescriptor, Tapret, TerminalPath};
    pub use runtime::{Runtime, RuntimeError};
    pub use wallet::{BlockchainResolver, DefaultResolver, RgbWallet};

    pub use rgbwallet::*;
    pub use rgbstd::*;
    pub use rgbfs::StockFs;
}
pub use prelude::*;
