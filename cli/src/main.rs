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

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate clap;

pub mod opts;
pub mod args;
pub mod cmd;
mod exec;

use std::backtrace::Backtrace;
use std::fmt::Display;
use std::panic::set_hook;

use clap::Parser;

use crate::args::Args;

fn main() -> anyhow::Result<()> {
    set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            Some(s.to_string())
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            Some(s.clone())
        } else {
            info.payload()
                .downcast_ref::<&dyn Display>()
                .map(ToString::to_string)
        };
        if let Some(error) = payload {
            eprintln!("Abnormal program termination through panic.");
            eprintln!("Error: {error}");
            if let Some(location) = info.location() {
                eprintln!("Happened in {location}");
            }
        } else {
            eprintln!("Program {info}");
        }
        let backtrace = Backtrace::capture();
        eprintln!("Backtrace: {backtrace}");
    }));
    Args::parse().exec()
}
