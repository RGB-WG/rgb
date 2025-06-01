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

use std::convert::Infallible;
use std::fs;
use std::fs::File;
use std::str::FromStr;

use anyhow::Context;
use bpwallet::psbt::{PsbtConstructor, TxParams};
use bpwallet::{ConsensusEncode, Indexer, Outpoint, Psbt, PsbtVer, Wpkh, XpubDerivable};
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::{PaymentScript, PrefabBundle, WalletProvider};
use rgb::{CallScope, Consensus, CreateParams, Issuer};
use rgbp::descriptor::RgbDescr;
use rgbp::{ContractInfo, Owner};
use strict_types::StrictVal;

use crate::args::Args;
use crate::cmd::Cmd;
use crate::opts::WalletOpts;

impl Args {
    pub fn exec(&self) -> anyhow::Result<()> {
        self.check_data_dir()?;
        match &self.command {
            Cmd::Init { .. } => {
                // Do nothing; the directory is already initialized in `check_data_dir`
            }

            // =====================================================================================
            // I. Wallet management
            Cmd::Wallets => {
                let data_dir = self.data_dir();
                let mut count = 0usize;
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
                        count += 1;
                    }
                }
                if count == 0 {
                    eprintln!("No wallets found");
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
                Owner::create(provider, descr, self.network, true)
                    .expect("Unable to create wallet");
            }

            Cmd::Sync { wallet, resolver } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let indexer = self.indexer(resolver);
                runtime.sync(&indexer)?;
                println!();
            }

            Cmd::Fund { wallet } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let addr = runtime.wallet.next_address();
                println!("{addr}");
            }

            Cmd::Seals { wallet } => {
                let runtime = self.runtime(wallet);
                for utxo in runtime.wallet.utxos() {
                    println!("{utxo}");
                }
            }

            // =====================================================================================
            // II. Contract management
            Cmd::Contracts { issuers } => {
                let contracts = self.contracts();

                if *issuers {
                    #[allow(clippy::print_literal)]
                    if contracts.issuers_count() > 0 {
                        println!("Contract issuers:");
                        println!(
                            "{:<72}\t{:<32}\t{:<16}\t{}",
                            "Codex ID", "Codex name", "Standard", "Developer"
                        );
                    } else {
                        eprintln!("No contract issuers found");
                    }
                    // TODO: Print codex and API information in separate blocks
                    for (codex_id, issuer) in contracts.issuers() {
                        let api = &issuer.default_api();
                        println!(
                            "{:<72}\t{:<32}\t{:<16}\t{}",
                            codex_id.to_string(),
                            issuer.codex_name(),
                            api.conforms()
                                .iter()
                                .map(|no| format!("RGB-{no}"))
                                .collect::<Vec<_>>()
                                .join(", "),
                            issuer.codex().developer,
                        );
                    }
                    println!();
                }

                if contracts.contracts_count() == 0 {
                    eprintln!("No contracts found");
                } else {
                    println!("Contracts:");
                }
                for id in contracts.contract_ids() {
                    let articles = contracts.contract_articles(id);
                    let info = ContractInfo::new(id, &articles);
                    println!("---");
                    println!(
                        "{}",
                        serde_yaml::to_string(&info).context("Unable to generate YAML")?
                    );
                }
            }

            Cmd::Issue { params: None, wallet: _, quiet: _ } => {
                println!(
                    "To issue a new contract, please specify a parameter file. A contract may be \
                     issued under one of the codex listed below."
                );
                println!();
                println!("{:<32}\t{:<64}\tDeveloper", "Name", "ID");
                for (codex_id, issuer) in self.contracts().issuers() {
                    println!(
                        "{:<32}\t{codex_id}\t{}",
                        issuer.codex_name(),
                        issuer.codex().developer
                    );
                }
            }
            Cmd::Issue { params: Some(params), wallet, quiet } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let file = File::open(params).context("Unable to open the parameter file")?;
                let params = serde_yaml::from_reader::<_, CreateParams<Outpoint>>(file)?;
                match runtime.issue(params) {
                    Ok(contract_id) => println!("A new contract issued with ID {contract_id}"),
                    Err(_err) if *quiet => return Ok(()),
                    Err(err) => return Err(err.into()),
                };
            }

            Cmd::Purge { contract } => {
                self.contracts().purge(*contract)?;
            }

            Cmd::Export { force, codex, file } => {
                let contracts = self.contracts();
                let issuer = contracts
                    .issuer(*codex)
                    .ok_or(anyhow::anyhow!("unknown issuer '{codex}'"))?;
                if *force && fs::exists(file)? {
                    fs::remove_file(file)?;
                }
                issuer.save(file)?;
            }

            Cmd::Import { file: files } => {
                for src in files {
                    let mut contracts = self.contracts();
                    let Some(filename) = src.file_name() else {
                        eprintln!("Warning: '{}' is not a file, ignoring", src.display());
                        continue;
                    };
                    print!("Processing '{}' ... ", filename.to_string_lossy());

                    let issuer = Issuer::load(src, |_, _, _| Result::<_, Infallible>::Ok(()))?;
                    let codex_id = issuer.codex_id();
                    print!("codex id {codex_id} ... ");
                    if contracts.has_issuer(codex_id) {
                        println!("already known, skipping");
                        continue;
                    }
                    contracts.import_issuer(issuer)?;
                    println!("success");
                }
            }

            Cmd::Backup { force, contract, file } => {
                let contracts = self.contracts();
                let contract_id = contracts
                    .find_contract_id(contract.clone())
                    .ok_or(anyhow::anyhow!("unknown contract '{contract}'"))?;
                if *force && fs::exists(file)? {
                    fs::remove_file(file)?;
                }
                contracts.export_to_file(file, contract_id)?;
            }

            // =====================================================================================
            // III. Combined contract/wallet operations
            Cmd::State { wallet, all, global, owned, contract } => {
                let mut runtime = self.runtime(wallet);
                if wallet.sync {
                    let indexer = self.indexer(&wallet.resolver);
                    runtime.sync(&indexer)?;
                    println!();
                }
                let contract_id = contract
                    .as_ref()
                    .map(|r| {
                        runtime
                            .contracts
                            .find_contract_id(r.clone())
                            .ok_or(anyhow::anyhow!("unknown contract '{r}'"))
                    })
                    .transpose()?;
                let contract_ids = if let Some(contract_id) = contract_id {
                    set![contract_id]
                } else {
                    runtime.contracts.contract_ids().collect()
                };
                for contract_id in contract_ids {
                    let state = if *all {
                        runtime.state_all(contract_id).map(|seal| seal.primary)
                    } else {
                        runtime.state_own(contract_id)
                    };
                    let articles = runtime.contracts.contract_articles(contract_id);
                    println!("{contract_id}\t{}", articles.issue().meta.name);
                    if *global {
                        if state.immutable.is_empty() {
                            println!("Global: # no known global state");
                        } else {
                            println!(
                                "Global: {:<16}\t{:<12}\t{:<32}\t{:<32}\tRGB output",
                                "State name",
                                "Conf. height",
                                "Verifiable state",
                                "Unverifiable state"
                            );
                        }
                        for (name, map) in &state.immutable {
                            for state in map {
                                print!("\t{:<16}", name.as_str());
                                print!("\t{:<12}", state.status.to_string());
                                print!("\t{:<32}", state.data.verified.to_string());
                                if let Some(unverified) = &state.data.unverified {
                                    print!("\t{:<32}", unverified.to_string());
                                } else {
                                    print!("\t{:<32}", "~")
                                }
                                println!("\t{}", state.addr);
                            }
                        }

                        if state.aggregated.is_empty() {
                            println!("Aggr.:  # no known aggregated state");
                        } else {
                            print!("Aggr.:  {:<16}\t{:<32}", "State name", "Value");
                        }
                        for (name, val) in &state.aggregated {
                            println!("\t{name:<16}\t{val}");
                        }
                    }
                    if *owned {
                        if state.owned.is_empty() {
                            println!("Owned:  # no known owned state");
                        } else {
                            println!(
                                "Owned:  {:<16}\t{:<12}\t{:<32}\t{:<46}\tBitcoin outpoint",
                                "State name", "Conf. height", "Value", "RGB output"
                            );
                        }
                        for (name, map) in &state.owned {
                            for state in map {
                                print!("\t{:<16}", name.as_str());
                                print!("\t{:<12}", state.status.to_string());
                                print!("\t{:<32}", state.assignment.data.to_string());
                                print!("\t{:<46}", state.addr);
                                println!("\t{}", state.assignment.seal);
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
                        .contracts
                        .find_contract_id(contract.clone())
                        .ok_or(anyhow::anyhow!("unknown contract '{contract}'"))?;
                    CallScope::ContractId(id)
                } else {
                    CallScope::ContractQuery(s!(""))
                };
                // TODO: Support parsing values with the new StrictTypes update
                let value = value.map(StrictVal::num);
                let mut invoice = RgbInvoice::new(
                    contract_id,
                    Consensus::Bitcoin,
                    self.network.is_testnet(),
                    beneficiary,
                    value,
                );
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
                psbt2,
                print,
                force,
                psbt: psbt_filename,
                consignment: consignment_path,
            } => {
                let mut runtime = self.runtime(wallet);
                // TODO: sync wallet if needed
                // TODO: Add params and giveway to arguments
                let params = TxParams::with(*fee);
                let (mut psbt, payment) = runtime.pay_invoice(invoice, *strategy, params, *sats)?;
                let ver = if *psbt2 { PsbtVer::V2 } else { PsbtVer::V0 };

                let psbt_filename = psbt_filename
                    .as_ref()
                    .unwrap_or(consignment_path)
                    .with_extension("psbt");
                let mut psbt_file = if *force {
                    File::create(psbt_filename).context("Unable to create PSBT")?
                } else {
                    File::create_new(psbt_filename)
                        .context("Unable to create PSBT; try `--force`")?
                };
                psbt.encode(ver, &mut psbt_file)?;
                if *print {
                    psbt.version = ver;
                    println!("{psbt}");
                }
                if *force {
                    let _ = fs::remove_file(consignment_path);
                }
                runtime
                    .contracts
                    .consign_to_file(consignment_path, invoice.scope, payment.terminals)
                    .context("Unable to create the consignment file; try `--force`")?;
            }

            Cmd::Script { wallet, sats, strategy, invoice, output } => {
                let mut runtime = self.runtime(wallet);
                let script = runtime.script(invoice, *strategy, *sats)?;
                let file = File::create_new(output).context("Unable to open script file")?;
                serde_yaml::to_writer(file, &script).context("Unable to write script")?;
            }

            Cmd::Exec {
                wallet,
                print,
                script,
                fee,
                psbt2,
                bundle: bundle_filename,
                psbt: psbt_filename,
            } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let src = File::open(script).context("Unable to open script file")?;
                let script = serde_yaml::from_reader::<_, PaymentScript>(src)?;

                let params = TxParams::with(*fee);
                let mut payment = runtime.exec(script, params)?;
                let psbt_filename = psbt_filename
                    .as_ref()
                    .unwrap_or(bundle_filename)
                    .with_extension("psbt");
                let mut psbt_file =
                    File::create_new(psbt_filename).context("Unable to create PSBT")?;

                payment
                    .bundle
                    .save(bundle_filename)
                    .context("Unable to write to the output file")?;

                // This PSBT can be sent to other payjoin parties so they add their inputs and
                // outputs, or even re-order existing ones
                let ver = if *psbt2 { PsbtVer::V2 } else { PsbtVer::V0 };
                payment
                    .uncomit_psbt
                    .encode(ver, &mut psbt_file)
                    .context("Unable to write PSBT")?;
                if *print {
                    payment.uncomit_psbt.version = ver;
                    println!("{}", payment.uncomit_psbt);
                }
            }

            Cmd::Complete { wallet, bundle, psbt: psbt_filename } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                let bundle = PrefabBundle::load(bundle)?;
                let mut psbt_file = File::open(psbt_filename).context("Unable to open PSBT")?;
                let psbt = Psbt::decode(&mut psbt_file)?;

                let psbt = runtime.complete(psbt, &bundle)?;

                let mut psbt_file = File::create(psbt_filename).context("Unable to write PSBT")?;
                psbt.encode(psbt.version, &mut psbt_file)?;
            }

            Cmd::Consign { contract, terminals, output: consignment_path } => {
                let contracts = self.contracts();
                let contract_id = contracts
                    .find_contract_id(contract.clone())
                    .ok_or(anyhow::anyhow!("unknown contract '{contract}'"))?;
                contracts
                    .consign_to_file(consignment_path, contract_id, terminals)
                    .context("Unable to consign the contract")?;
            }

            Cmd::Finalize { broadcast, wallet, psbt: psbt_filename, tx: tx_filename } => {
                let runtime = self.runtime(wallet);
                let mut psbt_file = File::open(psbt_filename).context("Unable to open PSBT")?;
                let mut psbt = Psbt::decode(&mut psbt_file)?;
                let inputs = psbt.finalize(runtime.wallet.descriptor());
                eprint!("{inputs} of {} inputs were finalized", psbt.inputs().count());
                if psbt.is_finalized() {
                    eprintln!(", transaction is ready for the extraction");
                } else {
                    eprintln!(" and some non-finalized inputs remain");
                }

                eprint!("Extracting signed transaction ... ");
                let extracted = match psbt.extract() {
                    Ok(extracted) => {
                        eprintln!("success");
                        if !*broadcast && tx_filename.is_none() {
                            println!("{extracted}");
                        }
                        if let Some(file) = tx_filename {
                            eprint!("Saving transaction to file {} ...", file.display());
                            let mut file = File::create(file)?;
                            extracted.consensus_encode(&mut file)?;
                            eprintln!("success");
                        }
                        extracted
                    }
                    Err(e) if *broadcast || tx_filename.is_some() => {
                        anyhow::bail!(
                            "PSBT still contains {} non-finalized inputs, failing to extract \
                             transaction",
                            e.0
                        );
                    }
                    Err(e) => {
                        anyhow::bail!("{} more inputs still have to be finalized", e.0);
                    }
                };

                if *broadcast {
                    self.indexer(&wallet.resolver).broadcast(&extracted)?;
                }
            }

            Cmd::Accept { unknown, wallet, input } => {
                let mut runtime = self.runtime(&WalletOpts::default_with_name(wallet));
                runtime
                    .consume_from_file(*unknown, input, |_, _, _| Result::<_, Infallible>::Ok(()))
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
        }
        Ok(())
    }
}
