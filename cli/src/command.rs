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
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::confinement::U16;
use bp_util::{BpCommand, Config, Exec};
use bpstd::Sats;
use psbt::{Psbt, PsbtVer};
use rgb_rt::{DescriptorRgb, RgbKeychain, RuntimeError, TransferParams};
use rgbstd::containers::{Bindle, BuilderSeal, Transfer, UniversalBindle};
use rgbstd::contract::{ContractId, GenesisSeal, GraphSeal, StateType};
use rgbstd::interface::{AmountChange, ContractBuilder, FilterExclude, IfaceId, SchemaIfaces};
use rgbstd::invoice::{Beneficiary, RgbInvoice, RgbInvoiceBuilder, XChainNet};
use rgbstd::persistence::{Inventory, Stash};
use rgbstd::schema::SchemaId;
use rgbstd::validation::Validity;
use rgbstd::{OutputSeal, XChain, XOutputSeal};
use seals::txout::CloseMethod;
use strict_types::encoding::{FieldName, TypeName};
use strict_types::StrictVal;

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
    General(bp_util::Command),

    #[clap(flatten)]
    #[display(inner)]
    Debug(DebugCommand),

    /// Prints out list of known RGB schemata
    Schemata,
    /// Prints out list of known RGB interfaces
    Interfaces,
    /// Prints out list of known RGB contracts
    Contracts,

    /// Imports RGB data into the stash: contracts, schema, interfaces, etc
    #[display("import")]
    Import {
        /// Use BASE64 ASCII armoring for binary data
        #[arg(short)]
        armored: bool,

        /// File with RGB data
        ///
        /// If not provided, assumes `-a` and prints out data to STDOUT
        file: PathBuf,
    },

    /// Exports existing RGB contract
    #[display("export")]
    Export {
        /// Use BASE64 ASCII armoring for binary data
        #[arg(short)]
        armored: bool,

        /// Contract to export
        contract: ContractId,

        /// File with RGB data
        ///
        /// If not provided, assumes `-a` and reads the data from STDIN
        file: Option<PathBuf>,
    },

    /// Convert binary RGB file into a text armored version
    #[display("convert")]
    Armor {
        /// File with RGB data
        ///
        /// If not provided, assumes `-a` and reads the data from STDIN
        file: PathBuf,
    },

    /// Reports information about state of a contract
    #[display("state")]
    State {
        /// Show all state - not just the one owned by the wallet
        #[clap(short, long)]
        all: bool,

        /// Contract identifier
        contract_id: ContractId,

        /// Interface to interpret the state data
        iface: String,
    },

    /// Print operation history for a default fungible token under a given
    /// interface
    #[display("history-fungible")]
    HistoryFungible {
        /// Contract identifier
        contract_id: ContractId,

        /// Interface to interpret the state data
        iface: String,
    },

    /// Display all known UTXOs belonging to this wallet
    Utxos,

    /// Issues new contract
    #[display("issue")]
    Issue {
        /// Schema name to use for the contract
        schema: SchemaId, //String,

        /// File containing contract genesis description in YAML format
        contract: PathBuf,
    },

    /// Create new invoice
    #[display("invoice")]
    Invoice {
        /// Force address-based invoice
        #[clap(short, long)]
        address_based: bool,

        /// Contract identifier
        contract_id: ContractId,

        /// Interface to interpret the state data
        iface: String,

        /// Value to transfer
        value: u64,
    },

    /// Prepare PSBT file for transferring RGB assets. In the most of cases you
    /// need to use `transfer` command instead of `prepare` and `consign`.
    #[display("prepare")]
    Prepare {
        /// Encode PSBT as V2
        #[clap(short = '2')]
        v2: bool,

        /// Method for single-use-seals
        #[clap(long, default_value = "tapret1st")]
        method: CloseMethod,

        /// Amount of satoshis which should be paid to the address-based
        /// beneficiary
        #[clap(long, default_value = "2000")]
        sats: Sats,

        /// Invoice data
        invoice: RgbInvoice,

        /// Fee
        fee: Sats,

        /// Name of PSBT file to save. If not given, prints PSBT to STDOUT
        psbt: Option<PathBuf>,
    },

    /// Prepare consignment for transferring RGB assets. In the most of cases
    /// you need to use `transfer` command instead of `prepare` and `consign`.
    #[display("prepare")]
    Consign {
        /// Invoice data
        invoice: RgbInvoice,

        /// Name of PSBT file containing prepared transfer data
        psbt: PathBuf,

        /// File for generated transfer consignment
        consignment: PathBuf,
    },

    /// Transfer RGB assets
    #[display("transfer")]
    Transfer {
        /// Encode PSBT as V2
        #[clap(short = '2')]
        v2: bool,

        /// Method for single-use-seals
        #[clap(long, default_value = "tapret1st")]
        method: CloseMethod,

        /// Amount of satoshis which should be paid to the address-based
        /// beneficiary
        #[clap(long, default_value = "2000")]
        sats: Sats,

        /// Invoice data
        invoice: RgbInvoice,

        /// Fee for bitcoin transaction, in satoshis
        #[clap(short, long, default_value = "400")]
        fee: Sats,

        /// File for generated transfer consignment
        consignment: PathBuf,

        /// Name of PSBT file to save. If not given, prints PSBT to STDOUT
        psbt: Option<PathBuf>,
    },

    /// Inspects any RGB data file
    #[display("inspect")]
    Inspect {
        /// Format used for data inspection
        #[clap(short, long, default_value = "yaml")]
        format: InspectFormat,

        /// RGB file to inspect
        file: PathBuf,
    },

    /// Debug-dump all stash and inventory data
    #[display("dump")]
    Dump {
        /// Directory to put the dump into
        #[arg(default_value = "./rgb-dump")]
        root_dir: String,
    },

    /// Validate transfer consignment
    #[display("validate")]
    Validate {
        /// File with the transfer consignment
        file: PathBuf,
    },

    /// Validate transfer consignment & accept to the stash
    #[display("accept")]
    Accept {
        /// Force accepting consignments with non-mined terminal witness
        #[arg(short, long)]
        force: bool,

        /// File with the transfer consignment
        file: PathBuf,
    },
}

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
#[clap(hide = true)]
pub enum DebugCommand {
    Taprets,
}

impl Exec for RgbArgs {
    type Error = RuntimeError;
    const CONF_FILE_NAME: &'static str = "rgb.toml";

    fn exec(self, config: Config, _name: &'static str) -> Result<(), RuntimeError> {
        if let Some(mut runtime) = match &self.command {
            Command::General(cmd) => {
                self.inner.translate(cmd).exec(config, "rgb")?;
                None
            }
            Command::Debug(DebugCommand::Taprets) => {
                let runtime = self.rgb_runtime(&config)?;
                for (witness_id, tapret) in runtime.taprets()? {
                    println!("{witness_id}\t{tapret}");
                }
                None
            }
            Command::Schemata => {
                let runtime = self.rgb_runtime(&config)?;
                for id in runtime.schema_ids()? {
                    print!("{id} ");
                    for iimpl in runtime.schema(id)?.iimpls.values() {
                        let iface = runtime.iface_by_id(iimpl.iface_id)?;
                        print!("{} ", iface.name);
                    }
                    println!();
                }
                None
            }
            Command::Interfaces => {
                let runtime = self.rgb_runtime(&config)?;
                for (id, name) in runtime.ifaces()? {
                    println!("{} {id}", name);
                }
                None
            }
            Command::Contracts => {
                let runtime = self.rgb_runtime(&config)?;
                for id in runtime.contract_ids()? {
                    println!("{id}");
                }
                None
            }

            Command::Utxos => {
                self.inner
                    .translate(&BpCommand::Balance {
                        addr: true,
                        utxo: true,
                    })
                    .exec(config, "rgb")?;
                None
            }

            Command::HistoryFungible { contract_id, iface } => {
                let runtime = self.rgb_runtime(&config)?;
                let iface: TypeName = tn!(iface.clone());
                let history = runtime.fungible_history(*contract_id, iface)?;
                println!("Amount\tCounterparty\tWitness Id");
                for (id, op) in history {
                    let (cparty, more) = match op.state_change {
                        AmountChange::Dec(_) => {
                            (op.beneficiaries.first(), op.beneficiaries.len().saturating_sub(1))
                        }
                        AmountChange::Zero => continue,
                        AmountChange::Inc(_) => {
                            (op.payers.first(), op.payers.len().saturating_sub(1))
                        }
                    };
                    let more = if more > 0 {
                        format!(" (+{more})")
                    } else {
                        s!("")
                    };
                    let cparty = cparty
                        .map(XOutputSeal::to_string)
                        .unwrap_or_else(|| s!("none"));
                    println!("{}\t{}{}\t{}", op.state_change, cparty, more, id);
                }
                None
            }

            Command::Import { armored, file } => {
                let mut runtime = self.rgb_runtime(&config)?;
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
                            let mut resolver = self.resolver()?;
                            let id = bindle.id();
                            let contract = bindle
                                .unbindle()
                                .validate(&mut resolver, self.general.network.is_testnet())
                                .map_err(|c| {
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
                Some(runtime)
            }
            Command::Export {
                armored: _,
                contract,
                file,
            } => {
                let runtime = self.rgb_runtime(&config)?;
                let bindle = runtime
                    .export_contract(*contract)
                    .map_err(|err| err.to_string())?;
                if let Some(file) = file {
                    // TODO: handle armored flag
                    bindle.save(file)?;
                    eprintln!("Contract {contract} exported to '{}'", file.display());
                } else {
                    println!("{bindle}");
                }
                None
            }

            Command::Armor { file } => {
                let bindle = UniversalBindle::load_file(file)?;
                println!("{bindle}");
                None
            }

            Command::State {
                contract_id,
                iface,
                all,
            } => {
                let runtime = self.rgb_runtime(&config)?;

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
                                allocation.state, allocation.seal, allocation.witness
                            );
                        }
                    }
                    if *all {
                        if let Ok(allocations) =
                            contract.fungible(owned.name.clone(), &FilterExclude(&runtime))
                        {
                            for allocation in allocations {
                                println!(
                                    "    amount={}, utxo={}, witness={} # owner unknown",
                                    allocation.state, allocation.seal, allocation.witness
                                );
                            }
                        }
                    }
                    // TODO: Print out other types of state
                }
                None
            }
            Command::Issue { schema, contract } => {
                let mut runtime = self.rgb_runtime(&config)?;

                let file = fs::File::open(contract)?;

                let code = serde_yaml::from_reader::<_, serde_yaml::Value>(file)?;

                let code = code
                    .as_mapping()
                    .expect("invalid YAML root-level structure");

                let iface_name = code
                    .get("interface")
                    .expect("contract must specify interface under which it is constructed")
                    .as_str()
                    .expect("interface name must be a string");
                let SchemaIfaces {
                    ref schema,
                    ref iimpls,
                } = runtime.schema(*schema)?;
                let iface_name = tn!(iface_name.to_owned());
                let iface = runtime
                    .iface_by_name(&iface_name)
                    .or_else(|_| {
                        let id = IfaceId::from_str(iface_name.as_str())?;
                        runtime.iface_by_id(id).map_err(RuntimeError::from)
                    })?
                    .clone();
                let iface_id = iface.iface_id();
                let iface_impl = iimpls.get(&iface_id).ok_or_else(|| {
                    RuntimeError::Custom(format!(
                        "no known interface implementation for {iface_name}"
                    ))
                })?;
                let types = &schema.types;

                let mut builder = ContractBuilder::with(
                    iface.clone(),
                    schema.clone(),
                    iface_impl.clone(),
                    self.general.network.is_testnet(),
                )?;

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
                        let seal = OutputSeal::from_str(seal).expect("invalid seal definition");
                        let seal = GenesisSeal::new_random(seal.method, seal.txid, seal.vout);

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
                                let seal = BuilderSeal::Revealed(XChain::Bitcoin(seal));
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
                let mut resolver = self.resolver()?;
                let validated_contract = contract
                    .validate(&mut resolver, self.general.network.is_testnet())
                    .map_err(|consignment| {
                        RuntimeError::IncompleteContract(
                            consignment
                                .into_validation_status()
                                .expect("just validated"),
                        )
                    })?;
                runtime
                    .import_contract(validated_contract, &mut resolver)
                    .expect("failure importing issued contract");
                eprintln!(
                    "A new contract {id} is issued and added to the stash.\nUse `export` command \
                     to export the contract."
                );
                Some(runtime)
            }
            Command::Invoice {
                address_based,
                contract_id,
                iface,
                value,
            } => {
                let mut runtime = self.rgb_runtime(&config)?;
                let iface = TypeName::try_from(iface.to_owned()).expect("invalid interface name");

                let outpoint = runtime
                    .wallet()
                    .coinselect(Sats::ZERO, |utxo| {
                        RgbKeychain::contains_rgb(utxo.terminal.keychain)
                    })
                    .next();
                let network = runtime.wallet().network();
                let beneficiary = match (address_based, outpoint) {
                    (false, None) => {
                        return Err(RuntimeError::Custom(s!(
                            "blinded invoice requested but no suitable outpoint is available"
                        )));
                    }
                    (true, _) => {
                        let addr = runtime
                            .wallet()
                            .addresses(RgbKeychain::Rgb)
                            .next()
                            .expect("no addresses left")
                            .addr;
                        Beneficiary::WitnessVout(addr.payload)
                    }
                    (_, Some(outpoint)) => {
                        let seal = XChain::Bitcoin(GraphSeal::new_random(
                            runtime.wallet().seal_close_method(),
                            outpoint.txid,
                            outpoint.vout,
                        ));
                        runtime.store_seal_secret(seal)?;
                        Beneficiary::BlindedSeal(*seal.to_secret_seal().as_reduced_unsafe())
                    }
                };
                let invoice = RgbInvoiceBuilder::new(XChainNet::bitcoin(network, beneficiary))
                    .set_contract(*contract_id)
                    .set_interface(iface)
                    .set_amount_raw(*value)
                    .finish();
                println!("{invoice}");
                Some(runtime)
            }
            Command::Prepare {
                v2,
                method,
                invoice,
                fee,
                sats,
                psbt: psbt_file,
            } => {
                let mut runtime = self.rgb_runtime(&config)?;
                // TODO: Support lock time and RBFs
                let params = TransferParams::with(*fee, *sats);

                let (psbt, _) = runtime
                    .construct_psbt(invoice, *method, params)
                    .map_err(|err| err.to_string())?;

                let ver = if *v2 { PsbtVer::V2 } else { PsbtVer::V0 };
                match psbt_file {
                    Some(file_name) => {
                        let mut psbt_file = File::create(file_name)?;
                        psbt.encode(ver, &mut psbt_file)?;
                    }
                    None => match ver {
                        PsbtVer::V0 => println!("{psbt}"),
                        PsbtVer::V2 => println!("{psbt:#}"),
                    },
                }
                Some(runtime)
            }
            Command::Consign {
                invoice,
                psbt: psbt_name,
                consignment: out_file,
            } => {
                let mut runtime = self.rgb_runtime(&config)?;
                let mut psbt_file = File::open(psbt_name)?;
                let mut psbt = Psbt::decode(&mut psbt_file)?;
                let transfer = runtime
                    .transfer(invoice, &mut psbt)
                    .map_err(|err| err.to_string())?;
                let mut psbt_file = File::create(psbt_name)?;
                psbt.encode(psbt.version, &mut psbt_file)?;
                transfer.save(out_file)?;
                Some(runtime)
            }
            Command::Transfer {
                v2,
                method,
                invoice,
                fee,
                sats,
                psbt: psbt_file,
                consignment: out_file,
            } => {
                let mut runtime = self.rgb_runtime(&config)?;
                // TODO: Support lock time and RBFs
                let params = TransferParams::with(*fee, *sats);

                let (psbt, _, transfer) = runtime
                    .pay(invoice, *method, params)
                    .map_err(|err| err.to_string())?;

                transfer.save(out_file)?;

                let ver = if *v2 { PsbtVer::V2 } else { PsbtVer::V0 };
                match psbt_file {
                    Some(file_name) => {
                        let mut psbt_file = File::create(file_name)?;
                        psbt.encode(ver, &mut psbt_file)?;
                    }
                    None => match ver {
                        PsbtVer::V0 => println!("{psbt}"),
                        PsbtVer::V2 => println!("{psbt:#}"),
                    },
                }
                Some(runtime)
            }
            Command::Inspect { file, format } => {
                let bindle = UniversalBindle::load_file(file)?;
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
                None
            }
            Command::Dump { root_dir } => {
                let runtime = self.rgb_runtime(&config)?;

                fs::remove_dir_all(root_dir).ok();
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
                    for (no, suppl) in runtime
                        .contract_suppl_all(id)
                        .into_iter()
                        .flatten()
                        .enumerate()
                    {
                        fs::write(
                            format!("{root_dir}/stash/geneses/{id}.suppl.{no:03}.yaml"),
                            serde_yaml::to_string(suppl)?,
                        )?;
                    }
                    let tags = runtime.contract_asset_tags(id)?;
                    fs::write(
                        format!("{root_dir}/stash/geneses/{id}.tags.yaml"),
                        serde_yaml::to_string(tags)?,
                    )?;
                }
                for id in runtime.bundle_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/bundles/{id}.yaml"),
                        serde_yaml::to_string(runtime.bundle(id)?)?,
                    )?;
                }
                for id in runtime.witness_ids()? {
                    fs::write(
                        format!("{root_dir}/stash/anchors/{id}.debug"),
                        format!("{:#?}", runtime.anchor(id)?),
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
                None
            }
            Command::Validate { file } => {
                let mut resolver = self.resolver()?;
                let bindle = Bindle::<Transfer>::load_file(file)?;
                let consignment = bindle.unbindle();
                resolver.add_terminals(&consignment);
                let status =
                    match consignment.validate(&mut resolver, self.general.network.is_testnet()) {
                        Ok(consignment) => consignment.into_validation_status(),
                        Err(consignment) => consignment.into_validation_status(),
                    }
                    .expect("just validated");
                if status.validity() == Validity::Valid {
                    eprintln!("The provided consignment is valid")
                } else {
                    eprintln!("{status}");
                }
                None
            }
            Command::Accept { force, file } => {
                let mut runtime = self.rgb_runtime(&config)?;
                let mut resolver = self.resolver()?;
                let bindle = Bindle::<Transfer>::load_file(file)?;
                let consignment = bindle.unbindle();
                resolver.add_terminals(&consignment);
                let transfer = consignment
                    .validate(&mut resolver, self.general.network.is_testnet())
                    .unwrap_or_else(|c| c);
                eprintln!("{}", transfer.validation_status().expect("just validated"));
                runtime.accept_transfer(transfer, &mut resolver, *force)?;
                eprintln!("Transfer accepted into the stash");
                Some(runtime)
            }
        } {
            runtime.store()
        }

        println!();

        Ok(())
    }
}
