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
use bp::Tx;
use rgbstd::containers::{ContractBuilder, UniversalBindle};
use rgbstd::contract::{ContractId, GenesisSeal, StateType};
use rgbstd::interface::SchemaIfaces;
use rgbstd::persistence::{Inventory, Stock};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::schema::SchemaId;
use rgbstd::validation::{ResolveTx, TxResolverError};
use rgbstd::{Chain, Txid};
use strict_types::encoding::TypeName;
use strict_types::{StrictDumb, StrictVal};

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
    pub fn exec(self, stock: &mut Stock, chain: Chain) {
        match self {
            Self::Info => {
                println!("Schemata:");
                println!("---------");
                for (id, _item) in stock.schemata() {
                    println!("{id:#}");
                }

                println!("\nInterfaces:");
                println!("---------");
                for (id, item) in stock.ifaces() {
                    println!("{} {id:#}", item.name);
                }

                println!("\nContracts:");
                println!("---------");
                for (id, _item) in stock.contracts() {
                    println!("{id::<0}");
                }
            }
            Command::Import { armored, file } => {
                if armored {
                    todo!()
                } else {
                    let bindle = UniversalBindle::load(file).expect("invalid RGB file");
                    match bindle {
                        UniversalBindle::Iface(iface) => {
                            stock.import_iface(iface).expect("invalid interface")
                        }
                        UniversalBindle::Schema(schema) => {
                            stock.import_schema(schema).expect("invalid schema")
                        }
                        UniversalBindle::Impl(iimpl) => stock
                            .import_iface_impl(iimpl)
                            .expect("invalid interface implementation"),
                        UniversalBindle::Contract(contract) => stock
                            .import_contract(contract.unbindle(), &mut DumbResolver)
                            .expect("invalid contract"),
                        UniversalBindle::Transfer(_) => todo!(),
                    };
                }
            }
            Command::Export { .. } => {}
            Command::State { contract_id, iface } => {
                let iface = stock.iface(&iface).expect("invalid interface name").clone();
                let contract = stock
                    .contract_iface(contract_id, iface.iface_id())
                    .expect("unknown contract");

                let nominal = contract.global("Nominal").unwrap();
                let allocations = contract.fungible("Assets").unwrap();
                eprintln!("Global state:\nNominal:={}\n", nominal[0]);

                eprintln!("Owned state:");
                for (txout, amount) in allocations {
                    eprintln!("  (amount={amount}, txout={txout})");
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
                } = stock.schema(schema).expect("unknown schema");
                let iface = stock.iface(&iface).expect("invalid interface name").clone();
                let iface_id = iface.iface_id();
                let iface_impl = iimpls
                    .iter()
                    .find(|(id, _)| **id == iface_id)
                    .map(|(_, imp)| imp)
                    .expect("unknown interface implementation");
                let types = &schema.type_system;

                let file = fs::File::open(contract).expect("invalid contract file");

                let mut builder = ContractBuilder::with(iface, schema.clone(), iface_impl.clone())
                    .expect("schema fails to implement RGB20 interface")
                    .set_chain(chain);

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
                            .owned_state
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
                            .get("Seal")
                            .expect("assignment doesn't provide seal information")
                            .as_str()
                            .expect("seal must be a string");
                        let seal = GenesisSeal::from_str(seal).expect("invalid seal definition");

                        // Workaround for borrow checker:
                        let type_name =
                            TypeName::try_from(name.to_owned()).expect("invalid type name");
                        match state_schema.state_type() {
                            StateType::Void => todo!(),
                            StateType::Fungible => {
                                let amount = assign
                                    .get("Amount")
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

                let contract = builder.issue_contract().expect("failed issuing contract");
                let validated_contract = contract
                    .validate(&mut DumbResolver)
                    .expect("internal error: failed validating self-issued contract");
                stock
                    .import_contract(validated_contract, &mut DumbResolver)
                    .expect("failure importing issued contract");
            }
        }
    }
}
