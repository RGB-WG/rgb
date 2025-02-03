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

use std::fs::File;
use std::str::FromStr;

use anyhow::Context;
use bpwallet::psbt::TxParams;
use bpwallet::{Outpoint, Psbt, Sats, Wpkh, XpubDerivable};
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::{PaymentScript, PrefabBundle, WalletProvider};
use rgb::{CallScope, CreateParams};
use rgbp::descriptor::RgbDescr;
use rgbp::RgbWallet;
use strict_encoding::{StrictDeserialize, StrictSerialize};
use strict_types::StrictVal;

use crate::args::Args;
use crate::cmd::Cmd;
use crate::opts::WalletOpts;

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        match &self.command {
            // =====================================================================================
            // I. Wallet management
            Cmd::Wallets => {
                let data_dir = self.data_dir();
                for dir in data_dir
                    .read_dir()
                    .context("Unable to read data directory")?
                {
                    let dir = match dir {
                        Ok(dir) => dir,
                        Err(err) => {
                            eprintln!("Can't read directory entry: {err}");
                            continue;
                        }
                    };
                    let path = dir.path();
                    if dir
                        .file_type()
                        .context("Unable to read directory entry")?
                        .is_dir()
                        && path.extension() == Some("wallet".as_ref())
                    {
                        let Some(wallet_name) = path
                            .file_stem()
                            .context("Unable to parse wallet file name")?
                            .to_str()
                        else {
                            continue;
                        };
                        println!("{wallet_name}");
                    }
                }
            }

            Cmd::Create { tapret_key_only, wpkh, name, descriptor } => {
                let provider = self.wallet_provider(Some(name));
                let xpub = XpubDerivable::from_str(descriptor)?;
                let noise = xpub.xpub().chain_code().to_byte_array();
                let descr = match (tapret_key_only, wpkh) {
                    (false, true) => RgbDescr::new_unfunded(Wpkh::from(xpub), noise),
                    (true, false) => RgbDescr::key_only_unfunded(xpub, noise),
                    (true, true) => unreachable!(),
                    (false, false) => anyhow::bail!(
                        "a type of wallet descriptor must be specified either with \
                         `--tapret-key-only` or `--wpkh`"
                    ),
                };
                RgbWallet::create(provider, descr, self.network, true)
                    .expect("Unable to create wallet");
            }

            Cmd::Sync { wallet, resolver } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let indexer = self.indexer(resolver);
                runtime.wallet.update(&indexer, false);
                println!();
            }

            Cmd::Fund { wallet } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let addr = runtime.wallet.next_address();
                println!("{addr}");
            }

            Cmd::Seals { wallet } => {
                let runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                for utxo in runtime.wallet.utxos() {
                    println!("{utxo}");
                }
            }

            // =====================================================================================
            // II. Contract management
            Cmd::Contracts => {
                let mound = self.mound();
                for info in mound.contracts_info() {
                    println!("---");
                    println!(
                        "{}",
                        serde_yaml::to_string(&info).context("Unable to generate YAML")?
                    );
                }
            }

            Cmd::Issue { params: None, wallet: _ } => {
                println!(
                    "To issue a new contract please specify a parameters file. A contract may be \
                     issued under one of the codex listed below."
                );
                println!();
                println!("{:<32}\t{:<64}\tDeveloper", "Name", "ID");
                for (codex_id, schema) in self.mound().schemata() {
                    println!("{:<32}\t{codex_id}\t{}", schema.codex.name, schema.codex.developer);
                }
            }
            Cmd::Issue { params: Some(params), wallet } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let file = File::open(params).context("Unable to open parameters file")?;
                let params = serde_yaml::from_reader::<_, CreateParams<Outpoint>>(file)?;
                let contract_id = runtime.issue_to_file(params)?;
                println!("A new contract issued with ID {contract_id}");
            }

            Cmd::Purge { force, contract } => {
                todo!();
                //self.mound().purge(contract)
            }

            Cmd::Import { articles } => {
                todo!();
                //self.mound().import_file(articles)
            }

            Cmd::Export { contract, file } => {
                todo!();
                //self.mound().export_file(contract, file)
            }

            Cmd::Backup { file } => {
                todo!();
            }

            // =====================================================================================
            // III. Combined contract/wallet operations
            Cmd::State { wallet, all, global, owned, contract } => {
                let mut runtime = self.runtime(wallet);
                if wallet.sync {
                    let indexer = self.indexer(&wallet.resolver);
                    runtime.wallet.update(&indexer, false);
                    println!();
                }
                let contract_id = contract
                    .as_ref()
                    .map(|r| {
                        runtime
                            .mound
                            .find_contract_id(r.clone())
                            .ok_or(anyhow::anyhow!("unknown contract '{r}'"))
                    })
                    .transpose()?;
                let state = if *all {
                    runtime.state_all(contract_id).collect::<Vec<_>>()
                } else {
                    runtime.state_own(contract_id).collect()
                };
                for (contract_id, state) in state {
                    let contract = runtime.mound.contract(contract_id);
                    println!("{contract_id}\t{}", contract.stock().articles().contract.meta.name);
                    if *global {
                        if state.immutable.is_empty() {
                            println!("global: # no known global state is defined by the contract");
                        } else {
                            println!(
                                "global: {:<16}\t{:<32}\t{:<32}\taddress",
                                "state name", "verified state", "unverified state"
                            );
                        }
                        for (name, map) in &state.immutable {
                            let mut first = true;
                            for (addr, atom) in map {
                                print!("\t{:<16}", if first { name.as_str() } else { " " });
                                print!("\t{:<32}", atom.verified.to_string());
                                if let Some(unverified) = &atom.unverified {
                                    print!("\t{unverified:<32}");
                                } else {
                                    print!("\t{:<32}", "~")
                                }
                                println!("\t{addr}");
                                first = false;
                            }
                        }

                        if state.computed.is_empty() {
                            println!(
                                "comp:   # no known computed state is defined by the contract"
                            );
                        } else {
                            print!(
                                "comp:   {:<16}\t{:<32}\t{:<32}\taddress",
                                "state name", "verified state", "unverified state"
                            );
                        }
                        for (name, val) in &state.computed {
                            println!("\t{name:<16}\t{val}");
                        }
                    }
                    if *owned {
                        if state.owned.is_empty() {
                            println!("owned:  # no known owned state is defined by the contract");
                        } else {
                            println!(
                                "owned:  {:<16}\t{:<32}\t{:<46}\toutpoint",
                                "state name", "value", "address"
                            );
                        }
                        for (name, map) in &state.owned {
                            let mut first = true;
                            for (addr, assignment) in map {
                                print!("\t{:<16}", if first { name.as_str() } else { " " });
                                print!("\t{:<32}", assignment.data.to_string());
                                print!("\t{addr:<46}");
                                println!("\t{}", assignment.seal);
                                first = false;
                            }
                        }
                    }
                }
            }

            Cmd::Invoice {
                wallet,
                seal_only,
                wout,
                nonce,
                contract,
                api,
                method,
                state,
                value,
            } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let beneficiary = if *wout {
                    let wout = runtime.wout(*nonce);
                    RgbBeneficiary::WitnessOut(wout)
                } else {
                    let auth = runtime.auth_token(*nonce).ok_or(anyhow::anyhow!(
                        "Wallet has no unspent outputs; try `fund` first, or use `-w` flag to \
                         generate a witness output-based seal"
                    ))?;
                    RgbBeneficiary::Token(auth)
                };
                if *seal_only {
                    println!("{beneficiary}");
                    return Ok(());
                }

                let contract_id = if let Some(contract) = contract {
                    let id = runtime
                        .mound
                        .find_contract_id(contract.clone())
                        .ok_or(anyhow::anyhow!("unknown contract '{contract}'"))?;
                    CallScope::ContractId(id)
                } else {
                    CallScope::ContractQuery(s!(""))
                };
                let value = value.map(StrictVal::num);
                let mut invoice = RgbInvoice::new(contract_id, beneficiary, value);
                if let Some(api) = api {
                    invoice = invoice.use_api(api.clone());
                }
                if let Some(method) = method {
                    invoice = invoice.use_method(method.clone());
                }
                if let Some(state) = state {
                    invoice = invoice.use_state(state.clone());
                }

                println!("{invoice}");
            }

            Cmd::Pay {
                wallet,
                strategy,
                invoice,
                sats,
                fee,
                psbt: psbt_filename,
                consignment,
            } => {
                let mut runtime = self.runtime(wallet);
                // TODO: sync wallet if needed
                // TODO: Add params and giveway to arguments
                let params = TxParams::with(*fee);
                let (psbt, terminal) = runtime.pay_invoice(invoice, *strategy, params, *sats)?;
                if let Some(psbt_filename) = psbt_filename {
                    psbt.encode(
                        psbt.version,
                        &mut File::create(psbt_filename).context("Unable to write PSBT")?,
                    )?;
                } else {
                    println!("{psbt}");
                }
                runtime
                    .mound
                    .consign_to_file(invoice.scope, [terminal], consignment)
                    .context("Unable to consign contract")?;
            }

            Cmd::Script { wallet, strategy, invoice, output } => {
                let mut runtime = self.runtime(wallet);
                let giveaway = Some(Sats::from(500u16));
                let script = runtime.script(invoice, *strategy, giveaway)?;
                let file = File::create_new(output).context("Unable to open script file")?;
                serde_yaml::to_writer(file, &script).context("Unable to write script")?;
            }

            Cmd::Exec {
                wallet,
                script,
                fee,
                bundle: bundle_filename,
                psbt: psbt_filename,
                print,
            } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let src = File::open(script).context("Unable to open script file")?;
                let script = serde_yaml::from_reader::<_, PaymentScript>(src)?;

                let params = TxParams::with(*fee);
                let (psbt, bundle) = runtime.exec(script, params)?;
                let mut psbt_file = File::create_new(
                    psbt_filename
                        .as_ref()
                        .unwrap_or(bundle_filename)
                        .with_extension("psbt"),
                )
                .context("Unable to create PSBT")?;

                bundle
                    .strict_serialize_to_file::<{ usize::MAX }>(&bundle_filename)
                    .context("Unable to write output file")?;

                // This PSBT can be sent to other payjoin parties so they add their inputs and
                // outputs, or even re-order existing ones
                psbt.encode(psbt.version, &mut psbt_file)
                    .context("Unable to write PSBT")?;
                if *print {
                    println!("{psbt}");
                }
            }

            Cmd::Complete { wallet, bundle, psbt: psbt_filename } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let bundle = PrefabBundle::strict_deserialize_from_file::<{ usize::MAX }>(bundle)?;
                let mut psbt_file = File::open(psbt_filename).context("Unable to open PSBT")?;
                let psbt = Psbt::decode(&mut psbt_file)?;

                let psbt = runtime.complete(psbt, &bundle)?;

                let mut psbt_file = File::create(psbt_filename).context("Unable to write PSBT")?;
                psbt.encode(psbt.version, &mut psbt_file)?;
            }

            Cmd::Consign { contract, terminals, output } => {
                let mut mound = self.mound();
                let contract_id = mound
                    .find_contract_id(contract.clone())
                    .ok_or(anyhow::anyhow!("unknown contract '{contract}'"))?;
                mound
                    .consign_to_file(contract_id, terminals, output)
                    .context("Unable to consign contract")?;
            }

            Cmd::Accept { wallet, input } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                runtime.consume_from_file(input)?;
            }
        }
        Ok(())
    }
}
