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

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::confinement::U16;
use bp_util::{Config, Exec};
use bpstd::Txid;
use rgb::containers::{Bindle, Transfer, UniversalBindle};
use rgb::contract::{ContractId, GenesisSeal, GraphSeal, StateType};
use rgb::interface::{ContractBuilder, FilterExclude, SchemaIfaces, TypedState};
use rgb::persistence::{Inventory, Stash};
use rgb::schema::SchemaId;
use rgb_rt::{DescriptorRgb, RuntimeError};
use rgbinvoice::{RgbInvoice, RgbTransport};
use seals::txout::{CloseMethod, ExplicitSeal, TxPtr};
use strict_types::encoding::{FieldName, TypeName};
use strict_types::StrictVal;

use crate::resolver::PanickingResolver;
use crate::RgbArgs;

// TODO: For now, serde implementation doesn't work for consignments due to
//       some of the keys which can't be serialized to strings. Once this fixed,
//       allow this inspect formats option
#[derive(ValueEnum, Copy, Clone, Eq, PartialEq, Hash, Debug, Display, Default)]
#[display(lowercase)]
pub enum InspectFormat {
    #[default]
    Yaml,
    Toml,
    Json,
    Debug,
    Contractum,
}

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    #[clap(flatten)]
    #[display(inner)]
    Bp(bp_util::Command),

    /// Prints out list of known RGB schemata.
    Schemata,
    /// Prints out list of known RGB interfaces.
    Interfaces,
    /// Prints out list of known RGB contracts.
    Contracts,

    /// Imports RGB data into the stash: contracts, schema, interfaces, etc.
    #[display("import")]
    Import {
        /// Use BASE64 ASCII armoring for binary data.
        #[arg(short)]
        armored: bool,

        /// File with RGB data. If not provided, assumes `-a` and prints out
        /// data to STDOUT.
        file: PathBuf,
    },

    /// Exports existing RGB contract.
    #[display("export")]
    Export {
        /// Use BASE64 ASCII armoring for binary data.
        #[arg(short)]
        armored: bool,

        /// Contract to export.
        contract: ContractId,

        /// File with RGB data. If not provided, assumes `-a` and reads the data
        /// from STDIN.
        file: Option<PathBuf>,
    },

    /// Reports information about state of a contract.
    #[display("state")]
    State {
        /// Contract identifier.
        contract_id: ContractId,
        /// Interface to interpret the state data.
        iface: String,
    },

    /// Issues new contract.
    #[display("issue")]
    Issue {
        /// Schema name to use for the contract.
        schema: SchemaId, //String,

        /// Interface name to use for the contract.
        iface: String,

        /// File containing contract genesis description in YAML format.
        contract: PathBuf,
    },

    /// Create new invoice.
    #[display("invoice")]
    Invoice {
        /// Contract identifier.
        contract_id: ContractId,

        /// Interface to interpret the state data.
        iface: String,

        /// Value to transfer.
        value: u64,

        /// Seal to get the transfer to.
        seal: ExplicitSeal<TxPtr>,
    },

    /// Create new transfer.
    #[display("transfer")]
    Transfer {
        #[clap(long, default_value = "tapret1st")]
        /// Method for single-use-seals.
        method: CloseMethod,

        /// PSBT file.
        psbt_file: PathBuf,

        /// Invoice data.
        invoice: RgbInvoice,

        /// Filename to save transfer consignment.
        out_file: PathBuf,
    },

    /// Inspects any RGB data file.
    #[display("inspect")]
    Inspect {
        #[clap(short, long, default_value = "yaml")]
        /// Format used for data inspection
        format: InspectFormat,

        /// RGB file to inspect.
        file: PathBuf,
    },

    /// Debug-dump all stash and inventory data.
    #[display("dump")]
    Dump {
        /// Directory to put the dump into.
        #[arg(default_value = "./rgb-dump")]
        root_dir: String,
    },

    /// Validate transfer consignment.
    #[display("validate")]
    Validate {
        /// File with the transfer consignment.
        file: PathBuf,
    },

    /// Validate transfer consignment & accept to the stash.
    #[display("accept")]
    Accept {
        /// Force accepting consignments with non-mined terminal witness.
        #[arg(short, long)]
        force: bool,

        /// File with the transfer consignment.
        file: PathBuf,
    },

    /// Set first opret/tapret output to host a commitment
    #[display("set-host")]
    SetHost {
        #[arg(long, default_value = "tapret1st")]
        /// Method for single-use-seals.
        method: CloseMethod,

        /// PSBT file.
        psbt_file: PathBuf,
    },
}

impl Exec for RgbArgs {
    type Error = RuntimeError;
    const CONF_FILE_NAME: &'static str = "rgb.toml";

    fn exec(self, config: Config, name: &'static str) -> Result<(), RuntimeError> {
        match &self.command {
            Command::Bp(cmd) => {
                self.inner.translate(cmd).exec(config, "rgb")?;
            }
            Command::Schemata => {
                let runtime = self.rgb_runtime()?;
                for id in runtime.schema_ids()? {
                    print!("{id} ");
                    for iimpl in runtime.schema(id)?.iimpls.values() {
                        let iface = runtime.iface_by_id(iimpl.iface_id)?;
                        print!("{} ", iface.name);
                    }
                    println!();
                }
            }
            Command::Interfaces => {
                let runtime = self.rgb_runtime()?;
                for (id, name) in runtime.ifaces()? {
                    println!("{} {id}", name);
                }
            }
            Command::Contracts => {
                let runtime = self.rgb_runtime()?;
                for id in runtime.contract_ids()? {
                    println!("{id}");
                }
            }

            Command::Import { armored, file } => {
                let mut runtime = self.rgb_runtime()?;
                if *armored {
                    todo!()
                } else {
                    let bindle = UniversalBindle::load_file(file)?;
                    match bindle {
                        UniversalBindle::Iface(iface) => {
                            let id = iface.id();
                            let name = iface.name.clone();
                            runtime.import_iface(iface)?;
                            eprintln!("Interface {id} with name {name} imported to the stash");
                        }
                        UniversalBindle::Schema(schema) => {
                            let id = schema.id();
                            runtime.import_schema(schema)?;
                            eprintln!("Schema {id} imported to the stash");
                        }
                        UniversalBindle::Impl(iimpl) => {
                            let iface_id = iimpl.iface_id;
                            let schema_id = iimpl.schema_id;
                            let id = iimpl.id();
                            runtime.import_iface_impl(iimpl)?;
                            eprintln!(
                                "Implementation {id} of interface {iface_id} for schema \
                                 {schema_id} imported to the stash"
                            );
                        }
                        UniversalBindle::Contract(bindle) => {
                            let mut resolver = self.resolver();
                            let id = bindle.id();
                            let contract =
                                bindle.unbindle().validate(&mut resolver).map_err(|c| {
                                    c.validation_status().expect("just validated").to_string()
                                })?;
                            runtime.import_contract(contract, &mut resolver)?;
                            eprintln!("Contract {id} imported to the stash");
                        }
                        UniversalBindle::Transfer(_) => {
                            return Err(s!("use `validate` and `accept` commands to work with \
                                           transfer consignments")
                            .into());
                        }
                    };
                }
            }
            Command::Export {
                armored: _,
                contract,
                file,
            } => {
                let mut runtime = self.rgb_runtime()?;
                let bindle = runtime
                    .export_contract(*contract)
                    .map_err(|err| err.to_string())?;
                if let Some(file) = file {
                    // TODO: handle armored flag
                    bindle.save(&file)?;
                    eprintln!("Contract {contract} exported to '{}'", file.display());
                } else {
                    println!("{bindle}");
                }
            }

            Command::State { contract_id, iface } => {
                let mut runtime = self.rgb_runtime()?;
                let bp_runtime = self.bp_runtime::<DescriptorRgb>(&config)?;
                runtime.attach(bp_runtime.detach());

                let iface = runtime.iface_by_name(&tn!(iface.to_owned()))?.clone();
                let contract = runtime.contract_iface_id(*contract_id, iface.iface_id())?;

                println!("Global:");
                for global in &contract.iface.global_state {
                    if let Ok(values) = contract.global(global.name.clone()) {
                        for val in values {
                            println!("  {} := {}", global.name, val);
                        }
                    }
                }

                println!("\nOwned:");
                for owned in &contract.iface.assignments {
                    println!("  {}:", owned.name);
                    if let Ok(allocations) = contract.fungible(owned.name.clone(), &runtime) {
                        for allocation in allocations {
                            println!(
                                "    amount={}, utxo={}, witness={} # owned by the wallet",
                                allocation.value, allocation.owner, allocation.witness,
                            );
                        }
                    }
                    if let Ok(allocations) =
                        contract.fungible(owned.name.clone(), &FilterExclude(&runtime))
                    {
                        for allocation in allocations {
                            println!(
                                "    amount={}, utxo={}, witness={} # owner unknown",
                                allocation.value, allocation.owner, allocation.witness
                            );
                        }
                    }
                    // TODO: Print out other types of state
                }
            }
            Command::Issue {
                schema,
                iface: iface_name,
                contract,
            } => {
                let mut runtime = self.rgb_runtime()?;
                let SchemaIfaces {
                    ref schema,
                    ref iimpls,
                } = runtime.schema(*schema)?;
                let iface_name = tn!(iface_name.to_owned());
                let iface = runtime.iface_by_name(&iface_name)?.clone();
                let iface_id = iface.iface_id();
                let iface_impl = iimpls.get(&iface_id).ok_or_else(|| {
                    RuntimeError::Custom(format!(
                        "no known interface implementation for {iface_name}"
                    ))
                })?;
                let types = &schema.type_system;

                let file = fs::File::open(contract)?;

                let mut builder =
                    ContractBuilder::with(iface.clone(), schema.clone(), iface_impl.clone())?
                        .set_chain(runtime.chain());

                let code = serde_yaml::from_reader::<_, serde_yaml::Value>(file)?;

                let code = code
                    .as_mapping()
                    .expect("invalid YAML root-level structure");
                if let Some(globals) = code.get("globals") {
                    for (name, val) in globals
                        .as_mapping()
                        .expect("invalid YAML: globals must be an mapping")
                    {
                        let name = name
                            .as_str()
                            .expect("invalid YAML: global name must be a string");
                        let name = iface
                            .genesis
                            .global
                            .iter()
                            .find(|(n, _)| n.as_str() == name)
                            .and_then(|(_, spec)| spec.name.as_ref())
                            .map(FieldName::as_str)
                            .unwrap_or(name);
                        let state_type = iface_impl
                            .global_state
                            .iter()
                            .find(|info| info.name.as_str() == name)
                            .unwrap_or_else(|| panic!("unknown type name '{name}'"))
                            .id;
                        let sem_id = schema
                            .global_types
                            .get(&state_type)
                            .expect("invalid schema implementation")
                            .sem_id;
                        let val = StrictVal::from(val.clone());
                        let typed_val = types
                            .typify(val, sem_id)
                            .expect("global type doesn't match type definition");

                        let serialized = types
                            .strict_serialize_type::<U16>(&typed_val)
                            .expect("internal error");
                        // Workaround for borrow checker:
                        let field_name =
                            FieldName::try_from(name.to_owned()).expect("invalid type name");
                        builder = builder
                            .add_global_state(field_name, serialized)
                            .expect("invalid global state data");
                    }
                }

                if let Some(assignments) = code.get("assignments") {
                    for (name, val) in assignments
                        .as_mapping()
                        .expect("invalid YAML: assignments must be an mapping")
                    {
                        let name = name
                            .as_str()
                            .expect("invalid YAML: assignments name must be a string");
                        let name = iface
                            .genesis
                            .assignments
                            .iter()
                            .find(|(n, _)| n.as_str() == name)
                            .and_then(|(_, spec)| spec.name.as_ref())
                            .map(FieldName::as_str)
                            .unwrap_or(name);
                        let state_type = iface_impl
                            .assignments
                            .iter()
                            .find(|info| info.name.as_str() == name)
                            .expect("unknown type name")
                            .id;
                        let state_schema = schema
                            .owned_types
                            .get(&state_type)
                            .expect("invalid schema implementation");

                        let assign = val.as_mapping().expect("an assignment must be a mapping");
                        let seal = assign
                            .get("seal")
                            .expect("assignment doesn't provide seal information")
                            .as_str()
                            .expect("seal must be a string");
                        let seal =
                            ExplicitSeal::<Txid>::from_str(seal).expect("invalid seal definition");
                        let seal = GenesisSeal::from(seal);

                        // Workaround for borrow checker:
                        let field_name =
                            FieldName::try_from(name.to_owned()).expect("invalid type name");
                        match state_schema.state_type() {
                            StateType::Void => todo!(),
                            StateType::Fungible => {
                                let amount = assign
                                    .get("amount")
                                    .expect("owned state must be a fungible amount")
                                    .as_u64()
                                    .expect("fungible state must be an integer");
                                builder = builder
                                    .add_fungible_state(field_name, seal, amount)
                                    .expect("invalid global state data");
                            }
                            StateType::Structured => todo!(),
                            StateType::Attachment => todo!(),
                        }
                    }
                }

                let contract = builder.issue_contract().expect("failure issuing contract");
                let id = contract.contract_id();
                let mut resolver = PanickingResolver;
                let validated_contract = contract
                    .validate(&mut resolver)
                    .map_err(|_| RuntimeError::IncompleteContract)?;
                runtime
                    .import_contract(validated_contract, &mut resolver)
                    .expect("failure importing issued contract");
                eprintln!(
                    "A new contract {id} is issued and added to the stash.\nUse `export` command \
                     to export the contract."
                );
            }
            Command::Invoice {
                contract_id,
                iface,
                value,
                seal,
            } => {
                let mut runtime = self.rgb_runtime()?;
                let iface = TypeName::try_from(iface.to_owned()).expect("invalid interface name");
                let seal = GraphSeal::from(seal);
                let invoice = RgbInvoice {
                    transports: vec![RgbTransport::UnspecifiedMeans],
                    contract: Some(*contract_id),
                    iface: Some(iface),
                    operation: None,
                    assignment: None,
                    beneficiary: seal.to_concealed_seal().into(),
                    owned_state: TypedState::Amount(*value),
                    chain: None,
                    expiry: None,
                    unknown_query: none!(),
                };
                runtime.store_seal_secret(seal)?;
                println!("{invoice}");
            }
            Command::Transfer {
                method,
                psbt_file,
                invoice,
                out_file,
            } => {
                todo!()
                /*
                // TODO: Check PSBT format
                let psbt_data = fs::read(&psbt_file)?;
                let mut psbt = Psbt::deserialize(&psbt_data)?;
                let transfer = runtime
                    .pay(invoice, &mut psbt, method)
                    .map_err(|err| err.to_string())?;
                fs::write(&psbt_file, psbt.serialize())?;
                // TODO: Print PSBT as Base64
                transfer.save(&out_file)?;
                eprintln!("Transfer is created and saved into '{}'.", out_file.display());
                eprintln!(
                    "PSBT file '{}' is updated with all required commitments and ready to be \
                     signed.",
                    psbt_file.display()
                );
                eprintln!("Stash data are updated.");
                 */
            }
            Command::Inspect { file, format } => {
                let bindle = UniversalBindle::load(file)?;
                // TODO: For now, serde implementation doesn't work for consignments due to
                //       some of the keys which can't be serialized to strings. Once this fixed,
                //       allow this inspect formats option
                let s = match format {
                    InspectFormat::Yaml => {
                        serde_yaml::to_string(&bindle).expect("unable to present as YAML")
                    }
                    InspectFormat::Toml => {
                        toml::to_string(&bindle).expect("unable to present as TOML")
                    }
                    InspectFormat::Json => {
                        serde_json::to_string(&bindle).expect("unable to present as JSON")
                    }
                    InspectFormat::Debug => format!("{bindle:#?}"),
                    InspectFormat::Contractum => todo!("contractum representation"),
                };
                println!("{s}");
            }
            Command::Dump { root_dir } => {
                let runtime = self.rgb_runtime()?;

                fs::remove_dir_all(&root_dir).ok();
                fs::create_dir_all(format!("{root_dir}/stash/schemata"))?;
                fs::create_dir_all(format!("{root_dir}/stash/ifaces"))?;
                fs::create_dir_all(format!("{root_dir}/stash/geneses"))?;
                fs::create_dir_all(format!("{root_dir}/stash/bundles"))?;
                fs::create_dir_all(format!("{root_dir}/stash/anchors"))?;
                fs::create_dir_all(format!("{root_dir}/stash/extensions"))?;
                fs::create_dir_all(format!("{root_dir}/state"))?;
                fs::create_dir_all(format!("{root_dir}/index"))?;

                // Stash
                for id in runtime.schema_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/schemata/{id}.yaml"),
                        serde_yaml::to_string(runtime.schema(id)?)?,
                    )?;
                }
                for (id, name) in runtime.ifaces()? {
                    fs::write(
                        format!("{root_dir}/stash/ifaces/{id}.{name}.yaml"),
                        serde_yaml::to_string(runtime.iface_by_id(id)?)?,
                    )?;
                }
                for id in runtime.contract_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/geneses/{id}.yaml"),
                        serde_yaml::to_string(runtime.genesis(id)?)?,
                    )?;
                    for (no, suppl) in runtime.contract_suppl(id).into_iter().flatten().enumerate()
                    {
                        fs::write(
                            format!("{root_dir}/stash/geneses/{id}.suppl.{no:03}.yaml"),
                            serde_yaml::to_string(suppl)?,
                        )?;
                    }
                }
                for id in runtime.bundle_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/bundles/{id}.yaml"),
                        serde_yaml::to_string(runtime.bundle(id)?)?,
                    )?;
                }
                for id in runtime.anchor_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/anchors/{id}.yaml"),
                        serde_yaml::to_string(runtime.anchor(id)?)?,
                    )?;
                }
                for id in runtime.extension_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/extensions/{id}.yaml"),
                        serde_yaml::to_string(runtime.extension(id)?)?,
                    )?;
                }
                // TODO: Add sigs debugging

                // State
                for (id, history) in runtime.debug_history() {
                    fs::write(
                        format!("{root_dir}/state/{id}.yaml"),
                        serde_yaml::to_string(history)?,
                    )?;
                }

                // Index
                fs::write(
                    format!("{root_dir}/index/op-to-bundle.debug"),
                    format!("{:#?}", runtime.debug_bundle_op_index()),
                )?;
                fs::write(
                    format!("{root_dir}/index/bundle-to-anchor.debug"),
                    format!("{:#?}", runtime.debug_anchor_bundle_index()),
                )?;
                fs::write(
                    format!("{root_dir}/index/contracts.debug"),
                    format!("{:#?}", runtime.debug_contract_index()),
                )?;
                fs::write(
                    format!("{root_dir}/index/terminals.debug"),
                    format!("{:#?}", runtime.debug_terminal_index()),
                )?;
                fs::write(
                    format!("{root_dir}/seal-secret.debug"),
                    format!("{:#?}", runtime.debug_seal_secrets()),
                )?;
                eprintln!("Dump is successfully generated and saved to '{root_dir}'");
            }
            Command::Validate { file } => {
                let mut resolver = self.resolver();
                let bindle = Bindle::<Transfer>::load_file(file)?;
                let status = match bindle.unbindle().validate(&mut resolver) {
                    Ok(consignment) => consignment.into_validation_status(),
                    Err(consignment) => consignment.into_validation_status(),
                }
                .expect("just validated");
                eprintln!("{status}");
            }
            Command::Accept { force, file } => {
                let mut runtime = self.rgb_runtime()?;
                let mut resolver = self.resolver();
                let bindle = Bindle::<Transfer>::load_file(file)?;
                let transfer = bindle
                    .unbindle()
                    .validate(&mut resolver)
                    .unwrap_or_else(|c| c);
                eprintln!("{}", transfer.validation_status().expect("just validated"));
                runtime.accept_transfer(transfer, &mut resolver, *force)?;
                eprintln!("Transfer accepted into the stash");
            }
            Command::SetHost { method, psbt_file } => {
                todo!();
                /*
                let psbt_data = fs::read(&psbt_file)?;
                let mut psbt = Psbt::deserialize(&psbt_data)?;
                let mut psbt_modified = false;
                match method {
                    CloseMethod::OpretFirst => {
                        psbt.unsigned_tx
                            .output
                            .iter()
                            .zip(&mut psbt.outputs)
                            .find(|(o, outp)| {
                                o.script_pubkey.is_op_return() && !outp.is_opret_host()
                            })
                            .and_then(|(_, outp)| {
                                psbt_modified = true;
                                outp.set_opret_host().ok()
                            });
                    }
                    CloseMethod::TapretFirst => {
                        psbt.unsigned_tx
                            .output
                            .iter()
                            .zip(&mut psbt.outputs)
                            .find(|(o, outp)| {
                                o.script_pubkey.is_v1_p2tr() && !outp.is_tapret_host()
                            })
                            .and_then(|(_, outp)| {
                                psbt_modified = true;
                                outp.set_tapret_host().ok()
                            });
                    }
                    _ => {}
                };
                fs::write(&psbt_file, psbt.serialize())?;
                if psbt_modified {
                    eprintln!(
                        "PSBT file '{}' is updated with {method} host now set.",
                        psbt_file.display()
                    );
                }
                 */
            }
        }

        Ok(())
    }
}
