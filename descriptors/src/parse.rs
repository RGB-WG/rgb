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

use alloc::collections::BTreeSet;
use core::error::Error;
use core::fmt::Display;
use core::num::ParseIntError;
use core::str::FromStr;

use amplify::{hex, Bytes32};
use bpstd::compiler::{check_forms, DescrAst, DescrExpr, DescrParseError, NoKey, ScriptExpr};
use bpstd::dbc::tapret::TapretCommitment;
use bpstd::seals::{Noise, TxoSealExt, WOutpoint, WTxoSeal};
use bpstd::{
    DeriveSet, IndexParseError, Keychain, NormalIndex, Outpoint, OutpointParseError, StdDescr,
    Terminal, Tr, Vout, XkeyDecodeError, XkeyParseError,
};

use crate::{RgbDeriver, RgbDescr, SealDescr, TapretTweaks};

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum SealParseError {
    /// invalid seal literal '{0}' in position {1}.
    InvalidSeal(String, usize),

    /// failback information is absent from a seal definition in '{0}'.
    NoFailback(String),

    /// invalid primary outpoint in seal definition string. {0}
    InvalidPrimary(OutpointParseError),

    /// invalid fallback outpoint in seal definition string. {0}
    InvalidFallback(OutpointParseError),

    /// invalid noise data in seal definition. {0}
    InvalidNoise(String),

    #[from]
    /// invalid transaction output number in seal definition. {0}
    InvalidVout(ParseIntError),
}

impl From<XkeyDecodeError> for SealParseError {
    fn from(_: XkeyDecodeError) -> Self { unreachable!() }
}

impl FromStr for SealDescr {
    type Err = DescrParseError<SealParseError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ast = ScriptExpr::<NoKey>::from_str(s).map_err(DescrParseError::from)?;
        Self::parse_ast(ast)
    }
}

impl SealDescr {
    pub fn parse_ast<K: Display + FromStr>(
        ast: ScriptExpr<K>,
    ) -> Result<Self, DescrParseError<SealParseError>>
    where K::Err: Error {
        if ast.name == "seals" && ast.children.is_empty() {
            return Ok(SealDescr::default());
        }
        let form = check_forms(ast, "seals", &[DescrExpr::VariadicLit][..])
            .ok_or(DescrParseError::InvalidArgs("seals"))?;
        let mut set = bset![];
        for item in form {
            let DescrAst::Lit(s, _) = item else {
                unreachable!();
            };
            let (prim, sec) = s
                .split_once('/')
                .ok_or_else(|| SealParseError::NoFailback(s.to_owned()))
                .map_err(|e| DescrParseError::Expr("seal", e))?;
            let primary = if let Some(vout) = prim.strip_prefix("~:") {
                WOutpoint::Wout(Vout::from_str(vout)?)
            } else {
                WOutpoint::Extern(
                    Outpoint::from_str(prim)
                        .map_err(|e| SealParseError::InvalidPrimary(e))
                        .map_err(|e| DescrParseError::Expr("seal", e))?,
                )
            };
            let secondary = if sec.contains(':') {
                let fallback = Outpoint::from_str(sec)
                    .map_err(|e| SealParseError::InvalidFallback(e))
                    .map_err(|e| DescrParseError::Expr("seal", e))?;
                TxoSealExt::Fallback(fallback)
            } else {
                TxoSealExt::Noise(
                    Noise::from_str(sec)
                        .map_err(|_| SealParseError::InvalidNoise(sec.to_owned()))
                        .map_err(|e| DescrParseError::Expr("seal", e))?,
                )
            };
            let seal = WTxoSeal { primary, secondary };
            set.insert(seal);
        }
        Ok(SealDescr::from(set))
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum TweakParseError {
    /// invalid structure of tapret tweak expression '{0}'.
    InvalidStructure(String),

    #[from]
    /// invalid format of derivation index. {0}
    InvalidIndex(IndexParseError),

    /// invalid tapret tweak value '{0}'.
    InvalidTweak(String),
}

impl From<XkeyDecodeError> for TweakParseError {
    fn from(_: XkeyDecodeError) -> Self { unreachable!() }
}

impl FromStr for TapretTweaks {
    type Err = DescrParseError<TweakParseError>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ast = ScriptExpr::<NoKey>::from_str(s).map_err(DescrParseError::from)?;
        Self::parse_ast(ast)
    }
}

impl TapretTweaks {
    pub fn parse_ast<K: Display + FromStr>(
        ast: ScriptExpr<K>,
    ) -> Result<Self, DescrParseError<TweakParseError>>
    where K::Err: Error {
        if ast.name == "tapret" && ast.children.is_empty() {
            return Ok(TapretTweaks::default());
        }
        let form = check_forms(ast, "tweaks", &[DescrExpr::VariadicLit][..])
            .ok_or(DescrParseError::InvalidArgs("tweaks"))?;
        let mut map = bmap! {};
        for item in form {
            let DescrAst::Lit(s, _) = item else {
                unreachable!();
            };
            let mut split = s.split('/');
            if split.next() != Some("") {
                return Err(DescrParseError::Expr(
                    "tapret tweak",
                    TweakParseError::InvalidStructure(s.to_owned()),
                ));
            }
            let keychain = split
                .next()
                .ok_or_else(|| TweakParseError::InvalidStructure(s.to_owned()))
                .map_err(|e| DescrParseError::Expr("tapret tweak", e))?
                .parse::<Keychain>()?;
            let index = split
                .next()
                .ok_or_else(|| TweakParseError::InvalidStructure(s.to_owned()))
                .map_err(|e| DescrParseError::Expr("tapret tweak", e))?
                .parse::<NormalIndex>()
                .map_err(|e| DescrParseError::Expr("tapret tweak", e.into()))?;
            let Some(rest) = split.next() else {
                continue;
            };
            let term = Terminal::new(keychain, index);
            let set: &mut BTreeSet<TapretCommitment> = map.entry(term).or_default();
            if let Some(s) = rest.strip_prefix("<").and_then(|s| s.strip_suffix(">")) {
                for tweak in s.split(';') {
                    let tweak = TapretCommitment::from_str(tweak).map_err(|_| {
                        DescrParseError::Expr(
                            "tapret tweak",
                            TweakParseError::InvalidTweak(tweak.to_owned()),
                        )
                    })?;
                    set.insert(tweak);
                }
            } else {
                let tweak = TapretCommitment::from_str(rest).map_err(|_| {
                    DescrParseError::Expr(
                        "tapret tweak",
                        TweakParseError::InvalidTweak(rest.to_owned()),
                    )
                })?;

                set.insert(tweak);
            }
        }
        Ok(TapretTweaks::from(map))
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Display, Error, From)]
#[display(inner)]
pub enum RgbDescrParseError<E: Error> {
    #[from]
    Descr(DescrParseError<E>),

    #[from]
    Tweak(TweakParseError),

    #[from]
    Seal(SealParseError),

    #[from]
    Noise(hex::Error),
}

impl From<TweakParseError> for XkeyParseError {
    fn from(_: TweakParseError) -> Self {
        panic!("TweakParseError cannot be converted to XkeyParseError")
    }
}

impl From<SealParseError> for XkeyParseError {
    fn from(_: SealParseError) -> Self {
        panic!("TweakParseError cannot be converted to SealParseError")
    }
}

impl<E: Error> RgbDescrParseError<E> {
    pub fn from_tweak_err(err: DescrParseError<TweakParseError>) -> Self
    where E: From<TweakParseError> {
        match err {
            DescrParseError::Expr(_, err) => Self::Tweak(err),
            err => Self::Descr(DescrParseError::from(err)),
        }
    }

    pub fn from_seal_err(err: DescrParseError<SealParseError>) -> Self
    where E: From<SealParseError> {
        match err {
            DescrParseError::Expr(_, err) => Self::Seal(err),
            err => Self::Descr(DescrParseError::from(err)),
        }
    }
}

impl<K: DeriveSet + Display + FromStr> FromStr for RgbDeriver<K>
where
    K::Err: Error + From<TweakParseError>,
    K::Legacy: Display + FromStr<Err = K::Err>,
    K::Compr: Display + FromStr<Err = K::Err>,
    K::XOnly: Display + FromStr<Err = K::Err>,
{
    type Err = RgbDescrParseError<K::Err>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim_start().starts_with("tapret") {
            let ast = ScriptExpr::<K>::from_str(s).map_err(DescrParseError::from)?;
            Self::parse_ast(ast)
        } else {
            StdDescr::from_str(s)
                .map(Self::OpretOnly)
                .map_err(RgbDescrParseError::Descr)
        }
    }
}

impl<K: DeriveSet + Display + FromStr> RgbDeriver<K>
where
    K::Err: Error + From<TweakParseError>,
    K::Legacy: Display + FromStr<Err = K::Err>,
    K::Compr: Display + FromStr<Err = K::Err>,
    K::XOnly: Display + FromStr<Err = K::Err>,
{
    pub fn parse_ast(ast: ScriptExpr<K>) -> Result<Self, RgbDescrParseError<K::Err>> {
        if ast.name == "tapret" {
            let mut form = check_forms(ast, "tapret", &[DescrExpr::Script, DescrExpr::Script][..])
                .ok_or(DescrParseError::InvalidArgs("tapret"))?;

            let Some(DescrAst::Script(tweaks)) = form.pop() else {
                unreachable!();
            };
            let Some(DescrAst::Script(tr)) = form.pop() else {
                unreachable!();
            };

            let tweaks =
                TapretTweaks::parse_ast(*tweaks).map_err(RgbDescrParseError::from_tweak_err)?;
            let tr = Tr::from_str(tr.full)?;

            Ok(Self::Universal { tr, tweaks })
        } else {
            StdDescr::from_str(ast.full)
                .map(Self::OpretOnly)
                .map_err(RgbDescrParseError::Descr)
        }
    }
}

impl<K: DeriveSet + Display + FromStr> FromStr for RgbDescr<K>
where
    K::Err: Error + From<TweakParseError> + From<SealParseError>,
    K::Legacy: Display + FromStr<Err = K::Err>,
    K::Compr: Display + FromStr<Err = K::Err>,
    K::XOnly: Display + FromStr<Err = K::Err>,
{
    type Err = RgbDescrParseError<K::Err>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ast = ScriptExpr::<K>::from_str(s).map_err(DescrParseError::from)?;
        let mut form =
            check_forms(ast, "rgb", &[DescrExpr::Script, DescrExpr::Script, DescrExpr::Lit][..])
                .ok_or(DescrParseError::InvalidArgs("rgb"))?;

        let Some(DescrAst::Lit(noise, _)) = form.pop() else {
            unreachable!();
        };
        let Some(DescrAst::Script(seals)) = form.pop() else {
            unreachable!();
        };
        let Some(DescrAst::Script(descr)) = form.pop() else {
            unreachable!();
        };

        let deriver = RgbDeriver::parse_ast(*descr)?;
        let seals = SealDescr::parse_ast(*seals).map_err(RgbDescrParseError::from_seal_err)?;
        let noise = Bytes32::from_str(noise)?;

        Ok(Self { deriver, seals, noise, nonce: 0 })
    }
}
