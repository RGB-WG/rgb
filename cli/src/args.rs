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

#![allow(clippy::needless_update)] // Caused by the From derivation macro

use bp_util::{Config, DescriptorOpts};
use bpstd::{Wpkh, XpubDerivable};
use rgb_rt::{
    electrum, esplora_blocking, AnyResolver, AnyResolverError, RgbDescr, Runtime, RuntimeError,
    TapretKey,
};

use crate::Command;

#[derive(Args, Clone, PartialEq, Eq, Debug)]
#[group()]
pub struct DescrRgbOpts {
    /// Use tapret(KEY) descriptor as wallet.
    #[arg(long, global = true)]
    pub tapret_key_only: Option<XpubDerivable>,

    /// Use wpkh(KEY) descriptor as wallet.
    #[arg(long, global = true)]
    pub wpkh: Option<XpubDerivable>,
}

impl DescriptorOpts for DescrRgbOpts {
    type Descr = RgbDescr;

    fn is_some(&self) -> bool { self.tapret_key_only.is_some() || self.wpkh.is_some() }

    fn descriptor(&self) -> Option<Self::Descr> {
        self.tapret_key_only
            .clone()
            .map(TapretKey::from)
            .map(TapretKey::into)
            .or(self.wpkh.clone().map(Wpkh::from).map(Wpkh::into))
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
    pub inner: bp_util::Args<Command, DescrRgbOpts>,
}

impl Default for RgbArgs {
    fn default() -> Self { unreachable!() }
}

impl RgbArgs {
    pub fn rgb_runtime(&self, config: &Config) -> Result<Runtime, RuntimeError> {
        let bprt = self.inner.bp_runtime::<RgbDescr>(config)?;
        eprint!("Loading stock ... ");
        let runtime = Runtime::<RgbDescr>::load_attach(self.general.base_dir(), bprt)?;
        eprintln!("success");

        Ok(runtime)
    }

    #[allow(clippy::result_large_err)]
    pub fn resolver(&self) -> Result<AnyResolver, AnyResolverError> {
        if self.resolver.electrum != bp_util::DEFAULT_ELECTRUM {
            match electrum::Resolver::new(&self.resolver.electrum) {
                Ok(c) => Ok(AnyResolver::Electrum(Box::new(c))),
                Err(e) => Err(AnyResolverError::Electrum(e)),
            }
        } else {
            match esplora_blocking::Resolver::new(&self.resolver.esplora) {
                Ok(c) => Ok(AnyResolver::Esplora(Box::new(c))),
                Err(e) => Err(AnyResolverError::Esplora(e)),
            }
        }
    }
}
