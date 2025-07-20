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

use core::fmt;
use core::fmt::{Display, Formatter};

use crate::{SealDescr, TapretTweaks};

impl Display for SealDescr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("seals(")?;
        let mut iter = self.0.iter().peekable();
        while let Some(seal) = iter.next() {
            Display::fmt(seal, f)?;
            if iter.peek().is_some() {
                f.write_str(",")?;
            }
        }
        f.write_str(")")
    }
}

impl Display for TapretTweaks {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("tweaks(")?;
        let mut iter1 = self.iter().peekable();
        while let Some((term, tweaks)) = iter1.next() {
            if tweaks.is_empty() {
                continue;
            }
            write!(f, "/{}/{}/", term.keychain, term.index)?;
            if tweaks.len() > 1 {
                f.write_str("<")?;
            }
            let mut iter2 = tweaks.iter().peekable();
            while let Some(tweak) = iter2.next() {
                write!(f, "{tweak}")?;
                if iter2.peek().is_some() {
                    f.write_str(";")?;
                } else if tweaks.len() > 1 {
                    f.write_str(">")?;
                }
            }
            if iter1.peek().is_some() {
                f.write_str(",")?;
            }
        }
        f.write_str(")")
    }
}

#[cfg(test)]
mod test {
    use core::str::FromStr;

    use bpstd::dbc::tapret::TapretCommitment;
    use bpstd::seals::{TxoSealExt, WOutpoint, WTxoSeal};
    use bpstd::{Idx, Keychain, NormalIndex, Terminal, TrKey, Wpkh, XpubDerivable};
    use commit_verify::{Digest, Sha256};
    use rgb::{Outpoint, Txid};

    use crate::RgbDescr;

    fn base_descr() -> RgbDescr<XpubDerivable> {
        let s = "[643a7adc/86'/1'/0']tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*";
        let xpub = XpubDerivable::from_str(s).unwrap();
        RgbDescr::<XpubDerivable>::new_unfunded(Wpkh::from(xpub), [0xADu8; 32])
    }

    #[test]
    fn opret() {
        let descr = base_descr();
        assert_eq!(descr.to_string(), "rgb(\
            wpkh([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals()\
        )");
    }

    #[test]
    fn tapret() {
        let s = "[643a7adc/86'/1'/0']tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*";
        let xpub = XpubDerivable::from_str(s).unwrap();
        let mut descr = RgbDescr::<XpubDerivable>::new_unfunded(TrKey::from(xpub), [0xADu8; 32]);
        assert_eq!(descr.to_string(), "rgb(\
            tapret(\
                tr([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
                tweaks()\
            ),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals()\
        )");

        descr.add_tweak(
            Terminal::new(Keychain::OUTER, NormalIndex::ONE),
            TapretCommitment::from([0xBAu8; 33]),
        );
        assert_eq!(descr.to_string(), "rgb(\
            tapret(\
                tr([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
                tweaks(/0/1/xUGpuwjSUfFQ53BB6PCh36sjttPpYqXq6tNPXw2mC28mo)\
            ),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals()\
        )");

        descr.add_tweak(
            Terminal::new(Keychain::INNER, NormalIndex::ZERO),
            TapretCommitment::from([0xABu8; 33]),
        );
        assert_eq!(descr.to_string(), "rgb(\
            tapret(\
                tr([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
                tweaks(\
                    /0/1/xUGpuwjSUfFQ53BB6PCh36sjttPpYqXq6tNPXw2mC28mo,\
                    /1/0/szpHvMPBKt4t9PagDS68oqS8dUc1gZTUPFV5p9Wgh4rF4\
                )\
            ),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals()\
        )");

        descr.add_tweak(
            Terminal::new(Keychain::INNER, NormalIndex::ZERO),
            TapretCommitment::from([0x43u8; 33]),
        );
        assert_eq!(descr.to_string(), "rgb(\
            tapret(\
                tr([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
                tweaks(\
                    /0/1/xUGpuwjSUfFQ53BB6PCh36sjttPpYqXq6tNPXw2mC28mo,\
                    /1/0/<Lyundr2Zsp9JfZbDzSmkRTST46jD93CjGqkZkaBcZf96r;szpHvMPBKt4t9PagDS68oqS8dUc1gZTUPFV5p9Wgh4rF4>\
                )\
            ),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals()\
        )");
    }

    #[test]
    fn base_seals() {
        let mut descr = base_descr();

        let sha = Sha256::new();
        descr.add_seal(WTxoSeal::no_fallback(Outpoint::new(Txid::from([0xDE; 32]), 129), sha, 56));
        assert_eq!(descr.to_string(), "rgb(\
            wpkh([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals(dededededededededededededededededededededededededededededededede:129/F8GCQc9BuWAA7kGouPwxjZMA9WBNgFGFHG9kqYDNPFrN)\
        )");

        let sha = Sha256::new_with_prefix("test");
        descr.add_seal(WTxoSeal::no_fallback(Outpoint::new(Txid::from([0x13; 32]), 129), sha, 56));
        assert_eq!(descr.to_string(), "rgb(\
            wpkh([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals(\
                1313131313131313131313131313131313131313131313131313131313131313:129/397G2XyBYQZZX6YxnTHyJocEPszPQZTmnwBRQcpGuMCu,\
                dededededededededededededededededededededededededededededededede:129/F8GCQc9BuWAA7kGouPwxjZMA9WBNgFGFHG9kqYDNPFrN\
            )\
        )");
    }

    #[test]
    fn fallback_seal() {
        let mut descr = base_descr();

        let seal = WTxoSeal {
            primary: WOutpoint::Extern(Outpoint::new(Txid::from([0xDE; 32]), 129)),
            secondary: TxoSealExt::Fallback(Outpoint::new(Txid::from([0xAF; 32]), 2)),
        };
        descr.add_seal(seal);
        assert_eq!(descr.to_string(), "rgb(\
            wpkh([643a7adc/86h/1h/0h]tpubDCNiWHaiSkgnQjuhsg9kjwaUzaxQjUcmhagvYzqQ3TYJTgFGJstVaqnu4yhtFktBhCVFmBNLQ5sN53qKzZbMksm3XEyGJsEhQPfVZdWmTE2/<0;1>/*),\
            adadadadadadadadadadadadadadadadadadadadadadadadadadadadadadadad,\
            seals(dededededededededededededededededededededededededededededededede:129/afafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf:2)\
        )");
    }
}
