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

use std::convert::Infallible;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::confinement::U16;
use bitcoin::psbt::Psbt;
use bp::seals::txout::{CloseMethod, ExplicitSeal, TxPtr};
use bp::Tx;
use rgb::Runtime;
use rgbstd::containers::UniversalBindle;
use rgbstd::contract::{ContractId, GenesisSeal, GraphSeal, StateType};
use rgbstd::interface::{ContractBuilder, SchemaIfaces};
use rgbstd::persistence::{Inventory, Stash};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::schema::SchemaId;
use rgbstd::validation::{ResolveTx, TxResolverError};
use rgbstd::Txid;
use rgbwallet::{InventoryWallet, RgbInvoice, RgbTransport};
use strict_types::encoding::TypeName;
use strict_types::{StrictDumb, StrictVal};

// TODO: For now, serde implementation doesn't work for consignments due to
//       some of the keys which can't be serialized to strings. Once this fixed,
//       allow this inspect formats option
/*
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
 */

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display, Default)]
#[display(lowercase)]
pub enum Command {
    /// Prints out detailed information about RGB stash.
    #[default]
    #[clap(alias = "stash")]
    Info,

    /// Imports RGB data into the stash: contracts, schema, interfaces etc.
    #[display("import")]
    Import {
        /// Use BASE64 ASCII armoring for binary data.
        #[clap(short)]
        armored: bool,

        /// File with RGB data. If not provided, assumes `-a` and prints out
        /// data to STDOUT.
        file: PathBuf,
    },

    /// Exports existing RGB data from the stash.
    #[display("export")]
    Export {
        /// Use BASE64 ASCII armoring for binary data.
        #[clap(short)]
        armored: bool,

        /// File with RGB data. If not provided, assumes `-a` and reads the data
        /// from STDIN.
        file: Option<PathBuf>,
    },

    /// Reports information about state of a contact.
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
        // #[clap(short, long, default_value = "yaml")]
        // /// Format used for data inspection
        // format: InspectFormat,
        /// RGB file to inspect.
        file: PathBuf,
    },
}

struct DumbResolver;

impl ResolveTx for DumbResolver {
    fn resolve_tx(&self, _txid: Txid) -> Result<Tx, TxResolverError> { Ok(Tx::strict_dumb()) }
}

impl ResolveHeight for DumbResolver {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> Result<u32, Self::Error> { Ok(0) }
}

impl Command {
    pub fn exec(self, runtime: &mut Runtime) {
        match self {
            Self::Info => {
                println!("Schemata:");
                println!("---------");
                for id in runtime.schema_ids().expect("infallible") {
                    print!("{id:-}: ");
                    for iimpl in runtime
                        .schema(id)
                        .expect("internal inconsistency")
                        .iimpls
                        .values()
                    {
                        let iface = runtime
                            .iface_by_id(iimpl.iface_id)
                            .expect("interface not found");
                        print!("{} ", iface.name);
                    }
                    println!();
                }

                println!("\nInterfaces:");
                println!("---------");
                for (id, name) in runtime.ifaces().expect("infallible") {
                    println!("{} {id:-}", name);
                }

                println!("\nContracts:");
                println!("---------");
                for id in runtime.contract_ids().expect("infallible") {
                    println!("{id::<}");
                }
            }
            Command::Import { armored, file } => {
                if armored {
                    todo!()
                } else {
                    let bindle = UniversalBindle::load(file).expect("invalid RGB file");
                    match bindle {
                        UniversalBindle::Iface(iface) => {
                            runtime.import_iface(iface).expect("invalid interface")
                        }
                        UniversalBindle::Schema(schema) => {
                            runtime.import_schema(schema).expect("invalid schema")
                        }
                        UniversalBindle::Impl(iimpl) => runtime
                            .import_iface_impl(iimpl)
                            .expect("invalid interface implementation"),
                        UniversalBindle::Contract(bindle) => {
                            let contract = bindle
                                .unbindle()
                                .validate(&mut DumbResolver)
                                .expect("invalid contract");
                            runtime
                                .import_contract(contract, &mut DumbResolver)
                                .expect("invalid contract")
                        }
                        UniversalBindle::Transfer(bindle) => {
                            let transfer = bindle
                                .unbindle()
                                .validate(&mut DumbResolver)
                                .expect("invalid transfer");
                            runtime
                                .accept_transfer(transfer, &mut DumbResolver)
                                .expect("invalid transfer")
                        }
                    };
                }
            }
            Command::Export { armored, file } => {}
            Command::State { contract_id, iface } => {
                let iface = runtime
                    .iface_by_name(&tn!(iface))
                    .expect("invalid interface name")
                    .clone();
                let contract = runtime
                    .contract_iface(contract_id, iface.iface_id())
                    .expect("unknown contract");

                let nominal = contract.global("Nominal").unwrap();
                let allocations = contract.fungible("Assets").unwrap();
                eprintln!("Global state:\nNominal:={}\n", nominal[0]);

                eprintln!("Owned state:");
                for allocation in allocations {
                    eprintln!(
                        "  (amount={}, owner={}, witness={})",
                        allocation.value, allocation.owner, allocation.witness
                    );
                }
            }
            Command::Issue {
                schema,
                iface,
                contract,
            } => {
                let SchemaIfaces {
                    ref schema,
                    ref iimpls,
                } = runtime.schema(schema).expect("unknown schema");
                let iface = runtime
                    .iface_by_name(&tn!(iface))
                    .expect("invalid interface name")
                    .clone();
                let iface_id = iface.iface_id();
                let iface_impl = iimpls
                    .get(&iface_id)
                    .expect("unknown interface implementation");
                let types = &schema.type_system;

                let file = fs::File::open(contract).expect("invalid contract file");

                let mut builder = ContractBuilder::with(iface, schema.clone(), iface_impl.clone())
                    .expect("schema fails to implement RGB20 interface")
                    .set_chain(runtime.chain());

                let code = serde_yaml::from_reader::<_, serde_yaml::Value>(file)
                    .expect("invalid contract definition");

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
                        let state_type = iface_impl
                            .global_state
                            .iter()
                            .find(|info| info.name.as_str() == name)
                            .expect("unknown type name")
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
                        let type_name =
                            TypeName::try_from(name.to_owned()).expect("invalid type name");
                        builder = builder
                            .add_global_state(type_name, serialized)
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
                        let type_name =
                            TypeName::try_from(name.to_owned()).expect("invalid type name");
                        match state_schema.state_type() {
                            StateType::Void => todo!(),
                            StateType::Fungible => {
                                let amount = assign
                                    .get("amount")
                                    .expect("owned state must be a fungible amount")
                                    .as_u64()
                                    .expect("fungible state must be an integer");
                                builder = builder
                                    .add_fungible_state(type_name, seal, amount)
                                    .expect("invalid global state data");
                            }
                            StateType::Structured => todo!(),
                            StateType::Attachment => todo!(),
                        }
                    }
                }

                let contract = builder.issue_contract().expect("failure issuing contract");
                let validated_contract = contract
                    .validate(&mut DumbResolver)
                    .expect("internal error: failed validating self-issued contract");
                runtime
                    .import_contract(validated_contract, &mut DumbResolver)
                    .expect("failure importing issued contract");
            }
            Command::Invoice {
                contract_id,
                iface,
                value,
                seal,
            } => {
                let iface = TypeName::try_from(iface).expect("invalid interface name");
                let seal = GraphSeal::from(seal);
                let invoice = RgbInvoice {
                    transport: RgbTransport::UnspecifiedMeans,
                    contract: contract_id,
                    iface,
                    operation: None,
                    assignment: None,
                    beneficiary: seal.to_concealed_seal().into(),
                    value,
                    chain: None,
                    unknown_query: none!(),
                };
                runtime
                    .store_seal_secret(seal.blinding)
                    .expect("infallible");
                println!("{invoice}");
            }
            Command::Transfer {
                method,
                psbt_file,
                invoice,
                out_file,
            } => {
                // TODO: Check PSBT format
                let psbt_data = fs::read(&psbt_file).expect("unable to read PSBT file");
                let mut psbt = Psbt::deserialize(&psbt_data).expect("unable to parse PSBT file");
                let transfer = runtime
                    .pay(invoice, &mut psbt, method)
                    .expect("error paying invoice");
                fs::write(psbt_file, psbt.serialize()).expect("unable to write to PSBT file");
                // TODO: Print PSBT as Base64
                transfer
                    .save(out_file)
                    .expect("unable to write consignment to OUT_FILE");
            }
            Command::Inspect { file } => {
                let bindle = UniversalBindle::load(file).expect("invalid RGB file");
                // TODO: For now, serde implementation doesn't work for consignments due to
                //       some of the keys which can't be serialized to strings. Once this fixed,
                //       allow this inspect formats option
                /* let s = match format {
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
                 */
                println!("{bindle:#?}");
            }
        }
    }
}
