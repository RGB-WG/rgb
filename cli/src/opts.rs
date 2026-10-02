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

use clap::ValueHint;

use crate::cmd::RGB_WALLET_ENV;

pub const DEFAULT_ELECTRUM: &str = "mycitadel.io:50001";
pub const DEFAULT_ESPLORA: &str = "https://mempool.space/{network}/api";

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group(args = ["electrum", "esplora"])]
pub struct ResolverOpt {
    /// Electrum server to use
    #[arg(
        long,
        global = true,
        default_missing_value = DEFAULT_ELECTRUM,
        num_args = 0..=1,
        require_equals = true,
        env = "ELECRTUM_SERVER",
        value_hint = ValueHint::Url,
        value_name = "URL"
    )]
    pub electrum: Option<String>,

    /// Esplora server to use
    #[arg(
        long,
        global = true,
        default_missing_value = DEFAULT_ESPLORA,
        num_args = 0..=1,
        require_equals = true,
        env = "ESPLORA_SERVER",
        value_hint = ValueHint::Url,
        value_name = "URL"
    )]
    pub esplora: Option<String>,
}

#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct WalletOpts {
    /// Wallet to use
    #[clap(short, long, global = true, env = RGB_WALLET_ENV)]
    pub wallet: Option<String>,

    #[clap(long, global = true)]
    pub sync: bool,

    #[clap(flatten)]
    pub resolver: ResolverOpt,
}

impl WalletOpts {
    pub fn default_with_name(name: &Option<String>) -> Self {
        WalletOpts {
            wallet: name.clone(),
            sync: false,
            resolver: ResolverOpt { electrum: None, esplora: None },
        }
    }

    pub fn with(name: &Option<String>, resolver: &ResolverOpt) -> Self {
        WalletOpts {
            wallet: name.clone(),
            sync: false,
            resolver: resolver.clone(),
        }
    }
}
