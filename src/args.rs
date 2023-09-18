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

use bpstd::XpubDerivable;
use bpw::DescriptorOpts;
use rgb::descriptor::{DescriptorRgb, TapretKey};
use rgb_rt::{Runtime, RuntimeError};

use crate::Command;

#[derive(Args, Clone, PartialEq, Eq, Debug)]
pub struct DescrRgbOpts {
    /// Use tapret(KEY) descriptor as wallet.
    #[arg(long, global = true)]
    pub tapret_key_only: Option<XpubDerivable>,
}

impl DescriptorOpts for DescrRgbOpts {
    type Descr = DescriptorRgb;

    fn is_some(&self) -> bool { self.tapret_key_only.is_some() }

    fn descriptor(&self) -> Option<Self::Descr> {
        self.tapret_key_only
            .clone()
            .map(TapretKey::from)
            .map(TapretKey::into)
    }
}

/// Command-line arguments
#[derive(Parser)]
#[derive(Wrapper, WrapperMut, Clone, Eq, PartialEq, Debug, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
#[command(author, version, about)]
pub struct RgbArgs {
    #[clap(flatten)]
    #[from]
    #[wrap]
    pub inner: bpw::Args<Command, DescrRgbOpts>,
}

impl Default for RgbArgs {
    fn default() -> Self { unreachable!() }
}

impl RgbArgs {
    pub fn rgb_runtime(&self) -> Result<Runtime, RuntimeError> {
        eprint!("Loading stock ... ");
        let runtime = Runtime::<DescriptorRgb>::load_pure_rgb(
            self.general.data_dir.clone(),
            self.general.chain,
        )?;
        eprintln!("success");

        Ok(runtime)
    }
}
