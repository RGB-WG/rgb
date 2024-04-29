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

use amplify::confinement::{SmallOrdMap, TinyOrdMap, TinyOrdSet, U16 as MAX16};
use baid58::ToBaid58;
use bp_util::{BpCommand, Config, Exec};
use bpstd::Sats;
use psbt::{Psbt, PsbtVer};
use rgb_rt::{DescriptorRgb, RgbKeychain, RuntimeError, TransferParams};
use rgbstd::containers::{
    BuilderSeal, ContainerVer, ContentId, ContentSigs, Contract, FileContent, Terminal, Transfer,
    UniversalFile,
};
use rgbstd::contract::{ContractId, GenesisSeal, GraphSeal, StateType};
use rgbstd::interface::{AmountChange, ContractSuppl, FilterExclude, IfaceId};
use rgbstd::invoice::{Beneficiary, RgbInvoice, RgbInvoiceBuilder, XChainNet};
use rgbstd::persistence::fs::StoreFs;
use rgbstd::persistence::{SchemaIfaces, StashReadProvider};
use rgbstd::schema::SchemaId;
use rgbstd::validation::Validity;
use rgbstd::vm::RgbIsa;
use rgbstd::{BundleId, OutputSeal, XChain, XOutputSeal};
use seals::txout::CloseMethod;
use serde_crate::{Deserialize, Serialize};
use strict_types::encoding::{FieldName, TypeName};
use strict_types::StrictVal;

use crate::RgbArgs;

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
        /// RGB file to inspect
        file: PathBuf,

        /// Path to save the dumped data. If not given, prints PSBT to STDOUT.
        path: Option<PathBuf>,

        /// Export using directory format for the compound bundles
        #[clap(long, requires("path"))]
        dir: bool,
    },

    /// Reconstructs consignment from a YAML file
    #[display("reconstruct")]
    #[clap(hide = true)]
    Reconstruct {
        #[clap(long)]
        contract: bool,

        /// RGB file with the consignment YAML data
        src: PathBuf,

        /// Path for the resulting consignment file. If not given, prints the
        /// consignment to STDOUT.
        dst: Option<PathBuf>,
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
        if let Some(stock) = match &self.command {
            Command::General(cmd) => {
                self.inner.translate(cmd).exec(config, "rgb")?;
                None
            }
            Command::Debug(DebugCommand::Taprets) => {
                let stock = self.rgb_stock()?;
                for (witness_id, tapret) in stock.as_stash_provider().taprets()? {
                    println!("{witness_id}\t{tapret}");
                }
                None
            }
            Command::Schemata => {
                let stock = self.rgb_stock()?;
                for schema_iface in stock.schemata()? {
                    print!("{} ", schema_iface.schema.schema_id());
                    for iimpl in schema_iface.iimpls.values() {
                        let iface = stock.iface(iimpl.iface_id)?;
                        print!("{} ", iface.name);
                    }
                    println!();
                }
                None
            }
            Command::Interfaces => {
                let stock = self.rgb_stock()?;
                for (id, name) in stock.ifaces()? {
                    println!("{} {id}", name);
                }
                None
            }
            Command::Contracts => {
                let stock = self.rgb_stock()?;
                for id in stock.contract_ids()? {
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
                let mut stock = self.rgb_stock()?;
                assert!(!armored, "importing armored files is not yet supported");
                // TODO: Support armored files
                let content = UniversalFile::load_file(file)?;
                match content {
                    UniversalFile::Kit(kit) => {
                        let id = kit.kit_id();
                        eprintln!("Importing kit {id}");
                        let mut iface_names = map![];
                        let mut schema_names = map![];
                        for iface in &kit.ifaces {
                            let iface_id = iface.iface_id();
                            iface_names.insert(iface_id, &iface.name);
                            eprintln!("- Interface {} {}", iface.name, iface_id);
                        }
                        for schema in &kit.schemata {
                            let schema_id = schema.schema_id();
                            schema_names.insert(schema_id, &schema.name);
                            eprintln!("- Schema {} {}", schema.name, schema_id);
                        }
                        for iimpl in &kit.iimpls {
                            let iface = iface_names
                                .get(&iimpl.iface_id)
                                .map(|name| name.to_string())
                                .unwrap_or_else(|| iimpl.iface_id.to_string());
                            let schema = schema_names
                                .get(&iimpl.schema_id)
                                .map(|name| name.to_string())
                                .unwrap_or_else(|| iimpl.schema_id.to_string());
                            eprintln!("- Implementation of {iface} for {schema}",);
                        }
                        for lib in &kit.scripts {
                            eprintln!("- AluVM library {}", lib.id());
                        }
                        eprintln!("- Strict types: {} definitions", kit.types.len());
                        let kit = kit.validate().map_err(|(status, _)| status.to_string())?;
                        stock.import_kit(kit)?;
                        eprintln!("Kit is imported");
                    }
                    UniversalFile::Contract(contract) => {
                        let mut resolver = self.resolver()?;
                        let id = contract.consignment_id();
                        let contract = contract
                            .validate(&mut resolver, self.general.network.is_testnet())
                            .map_err(|(status, _)| status.to_string())?;
                        stock.import_contract(contract, &mut resolver)?;
                        eprintln!("Contract {id} is imported");
                    }
                    UniversalFile::Transfer(_) => {
                        return Err(s!("use `validate` and `accept` commands to work with \
                                       transfer consignments")
                        .into());
                    }
                }
                Some(stock)
            }
            Command::Export {
                armored: _,
                contract,
                file,
            } => {
                let stock = self.rgb_stock()?;
                let contract = stock
                    .export_contract(*contract)
                    .map_err(|err| err.to_string())?;
                if let Some(file) = file {
                    // TODO: handle armored flag
                    contract.save_file(file)?;
                    eprintln!("Contract {contract} exported to '{}'", file.display());
                } else {
                    println!("{contract}");
                }
                None
            }

            Command::Armor { file } => {
                let content = UniversalFile::load_file(file)?;
                println!("{content}");
                None
            }

            Command::State {
                contract_id,
                iface,
                all,
            } => {
                let runtime = self.rgb_runtime(&config)?;

                let iface = runtime.iface(tn!(iface.to_owned()))?.clone();
                let contract = runtime.contract_iface(*contract_id, iface.iface_id())?;

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
            Command::Issue {
                schema: schema_id,
                contract,
            } => {
                let mut stock = self.rgb_stock()?;

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
                } = stock.schema(*schema_id)?;
                let iface_name = tn!(iface_name.to_owned());
                let iface = stock
                    .iface(iface_name.clone())
                    .or_else(|_| {
                        let id = IfaceId::from_str(iface_name.as_str())?;
                        stock.iface(id).map_err(RuntimeError::from)
                    })?
                    .clone();
                let iface_id = iface.iface_id();
                let iface_impl = iimpls.get(&iface_id).ok_or_else(|| {
                    RuntimeError::Custom(format!(
                        "no known interface implementation for {iface_name}"
                    ))
                })?;

                let mut builder = stock.contract_builder(*schema_id, iface_id)?;
                let types = builder.type_system().clone();

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
                            .strict_serialize_type::<MAX16>(&typed_val)
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

                let contract = builder.issue_contract()?;
                let id = contract.contract_id();
                let mut resolver = self.resolver()?;
                stock.import_contract(contract, &mut resolver)?;
                eprintln!(
                    "A new contract {id} is issued and added to the stash.\nUse `export` command \
                     to export the contract."
                );
                Some(stock)
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
                        runtime.store_secret_seal(seal)?;
                        Beneficiary::BlindedSeal(*seal.to_secret_seal().as_reduced_unsafe())
                    }
                };
                let invoice = RgbInvoiceBuilder::new(XChainNet::bitcoin(network, beneficiary))
                    .set_contract(*contract_id)
                    .set_interface(iface)
                    .set_amount_raw(*value)
                    .finish();
                println!("{invoice}");
                Some(runtime.into_stock())
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
                Some(runtime.into_stock())
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
                transfer.save_file(out_file)?;
                Some(runtime.into_stock())
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

                transfer.save_file(out_file)?;

                let ver = if *v2 { PsbtVer::V2 } else { PsbtVer::V0 };
                match psbt_file {
                    Some(file_name) => {
                        let mut psbt_file = File::create(file_name)?;
                        psbt.encode(ver, &mut psbt_file)?;
                    }
                    None => match ver {
                        PsbtVer::V0 => println!("{psbt:0}"),
                        PsbtVer::V2 => println!("{psbt:2}"),
                    },
                }
                Some(runtime.into_stock())
            }
            Command::Inspect { file, dir, path } => {
                #[derive(Clone, Debug)]
                #[derive(Serialize, Deserialize)]
                #[serde(crate = "serde_crate", rename_all = "camelCase")]
                pub struct ConsignmentInspection {
                    version: ContainerVer,
                    transfer: bool,
                    terminals: SmallOrdMap<BundleId, Terminal>,
                    supplements: TinyOrdSet<ContractSuppl>,
                    signatures: TinyOrdMap<ContentId, ContentSigs>,
                }

                let content = UniversalFile::load_file(file)?;
                let consignment = match content {
                    UniversalFile::Contract(contract) if *dir => Some(contract),
                    UniversalFile::Transfer(transfer) if *dir => Some(transfer.into_contract()),
                    content => {
                        let s = serde_yaml::to_string(&content).expect("unable to present as YAML");
                        match path {
                            None => println!("{s}"),
                            Some(path) => fs::write(path, s)?,
                        }
                        None
                    }
                };
                if let Some(consignment) = consignment {
                    let mut map = map![
                        s!("genesis.yaml") => serde_yaml::to_string(&consignment.genesis)?,
                        s!("schema.yaml") => serde_yaml::to_string(&consignment.schema)?,
                        s!("bundles.yaml") => serde_yaml::to_string(&consignment.bundles)?,
                        s!("extensions.yaml") => serde_yaml::to_string(&consignment.extensions)?,
                        s!("types.sty") => consignment.types.to_string(),
                    ];
                    for lib in consignment.scripts {
                        let mut buf = Vec::new();
                        lib.print_disassemble::<RgbIsa>(&mut buf)?;
                        map.insert(format!("{}.aluasm", lib.id().to_baid58().mnemonic()), unsafe {
                            String::from_utf8_unchecked(buf)
                        });
                    }
                    for (iface, iimpl) in consignment.ifaces {
                        map.insert(
                            format!("iface-{}.yaml", iface.name),
                            serde_yaml::to_string(&iface)?,
                        );
                        map.insert(
                            format!("impl-{}.yaml", iface.name),
                            serde_yaml::to_string(&iimpl)?,
                        );
                    }
                    let contract = ConsignmentInspection {
                        version: consignment.version,
                        transfer: consignment.transfer,
                        terminals: consignment.terminals,
                        supplements: consignment.supplements,
                        signatures: consignment.signatures,
                    };
                    map.insert(s!("consignment-meta.yaml"), serde_yaml::to_string(&contract)?);
                    let path = path.as_ref().expect("required by clap");
                    fs::create_dir_all(path)?;
                    for (file, value) in map {
                        fs::write(format!("{}/{file}", path.display()), value)?;
                    }
                }
                None
            }
            Command::Reconstruct {
                contract: false,
                src,
                dst,
            } => {
                let file = File::open(src)?;
                let transfer: Transfer = serde_yaml::from_reader(&file)?;
                match dst {
                    None => println!("{transfer}"),
                    Some(dst) => {
                        transfer.save_file(dst)?;
                    }
                }
                None
            }
            Command::Reconstruct {
                contract: true,
                src,
                dst,
            } => {
                let file = File::open(src)?;
                let contract: Contract = serde_yaml::from_reader(&file)?;
                match dst {
                    None => println!("{contract}"),
                    Some(dst) => {
                        contract.save_file(dst)?;
                    }
                }
                None
            }
            Command::Dump { root_dir } => {
                let stock = self.rgb_stock()?;

                fs::remove_dir_all(root_dir).ok();
                fs::create_dir_all(format!("{root_dir}/stash/schemata"))?;
                fs::create_dir_all(format!("{root_dir}/stash/ifaces"))?;
                fs::create_dir_all(format!("{root_dir}/stash/geneses"))?;
                fs::create_dir_all(format!("{root_dir}/stash/bundles"))?;
                fs::create_dir_all(format!("{root_dir}/stash/witnesses"))?;
                fs::create_dir_all(format!("{root_dir}/stash/extensions"))?;
                fs::create_dir_all(format!("{root_dir}/state"))?;
                fs::create_dir_all(format!("{root_dir}/index"))?;

                // Stash
                for schema_ifaces in stock.schemata()? {
                    fs::write(
                        format!(
                            "{root_dir}/stash/schemata/{}.yaml",
                            schema_ifaces.schema.schema_id()
                        ),
                        serde_yaml::to_string(&schema_ifaces)?,
                    )?;
                }
                for (id, name) in stock.ifaces()? {
                    fs::write(
                        format!("{root_dir}/stash/ifaces/{id}.{name}.yaml"),
                        serde_yaml::to_string(stock.iface(id)?)?,
                    )?;
                }
                for (id, genesis) in stock.as_stash_provider().debug_geneses() {
                    fs::write(
                        format!("{root_dir}/stash/geneses/{id}.yaml"),
                        serde_yaml::to_string(genesis)?,
                    )?;
                }
                for (id, list) in stock.as_stash_provider().debug_suppl() {
                    for suppl in list {
                        fs::write(
                            format!(
                                "{root_dir}/stash/geneses/{id}.suppl.{}.yaml",
                                suppl.suppl_id()
                            ),
                            serde_yaml::to_string(suppl)?,
                        )?;
                    }
                }
                for (id, bundle) in stock.as_stash_provider().debug_bundles() {
                    fs::write(
                        format!("{root_dir}/stash/bundles/{id}.yaml"),
                        serde_yaml::to_string(bundle)?,
                    )?;
                }
                for (id, witness) in stock.as_stash_provider().debug_witnesses() {
                    fs::write(
                        format!("{root_dir}/stash/witnesses/{id}.yaml"),
                        serde_yaml::to_string(witness)?,
                    )?;
                }
                for (id, extension) in stock.as_stash_provider().debug_extensions() {
                    fs::write(
                        format!("{root_dir}/stash/extensions/{id}.yaml"),
                        serde_yaml::to_string(extension)?,
                    )?;
                }
                fs::write(
                    format!("{root_dir}/seal-secret.yaml"),
                    serde_yaml::to_string(stock.as_stash_provider().debug_secret_seals())?,
                )?;
                // TODO: Add sigs debugging

                // State
                for (id, history) in stock.as_state_provider().debug_history() {
                    fs::write(
                        format!("{root_dir}/state/{id}.yaml"),
                        serde_yaml::to_string(history)?,
                    )?;
                }

                // Index
                fs::write(
                    format!("{root_dir}/index/op-to-bundle.yaml"),
                    serde_yaml::to_string(stock.as_index_provider().debug_op_bundle_index())?,
                )?;
                fs::write(
                    format!("{root_dir}/index/bundle-to-contract.yaml"),
                    serde_yaml::to_string(stock.as_index_provider().debug_bundle_contract_index())?,
                )?;
                fs::write(
                    format!("{root_dir}/index/bundle-to-witness.yaml"),
                    serde_yaml::to_string(stock.as_index_provider().debug_bundle_witness_index())?,
                )?;
                fs::write(
                    format!("{root_dir}/index/contracts.yaml"),
                    serde_yaml::to_string(stock.as_index_provider().debug_contract_index())?,
                )?;
                fs::write(
                    format!("{root_dir}/index/terminals.yaml"),
                    serde_yaml::to_string(stock.as_index_provider().debug_terminal_index())?,
                )?;
                eprintln!("Dump is successfully generated and saved to '{root_dir}'");
                None
            }
            Command::Validate { file } => {
                let mut resolver = self.resolver()?;
                let consignment = Transfer::load_file(file)?;
                resolver.add_terminals(&consignment);
                let status =
                    match consignment.validate(&mut resolver, self.general.network.is_testnet()) {
                        Ok(consignment) => consignment.into_validation_status(),
                        Err((status, _)) => status,
                    };
                if status.validity() == Validity::Valid {
                    eprintln!("The provided consignment is valid")
                } else {
                    eprintln!("{status}");
                }
                None
            }
            Command::Accept { force: _, file } => {
                // TODO: Ensure we properly handle unmined terminal transactions
                let mut stock = self.rgb_stock()?;
                let mut resolver = self.resolver()?;
                let transfer = Transfer::load_file(file)?;
                resolver.add_terminals(&transfer);
                let valid = transfer
                    .validate(&mut resolver, self.general.network.is_testnet())
                    .map_err(|(status, _)| status)?;
                stock.accept_transfer(valid, &mut resolver)?;
                eprintln!("Transfer accepted into the stash");
                Some(stock)
            }
        } {
            stock
                .store(self.general.base_dir())
                .expect("unable to save stock");
        }

        println!();

        Ok(())
    }
}
