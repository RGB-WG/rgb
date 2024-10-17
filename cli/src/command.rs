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
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::str::FromStr;

use amplify::confinement::{SmallOrdMap, TinyOrdMap, TinyOrdSet};
use baid64::DisplayBaid64;
use bpstd::{Sats, XpubDerivable};
use bpwallet::Wallet;
use bpwallet::cli::{BpCommand, Config, Exec};
use psbt::{Psbt, PsbtVer};
use rgb::containers::{
    BuilderSeal, ConsignmentExt, ContainerVer, ContentId, ContentSigs, Contract, FileContent,
    Supplement, Transfer, UniversalFile,
};
use rgb::interface::{AssignmentsFilter, ContractOp, IfaceId};
use rgb::invoice::{Beneficiary, Pay2Vout, RgbInvoice, RgbInvoiceBuilder, XChainNet};
use rgb::persistence::{MemContract, StashReadProvider, Stock, StockError};
use rgb::resolvers::ContractIssueResolver;
use rgb::schema::SchemaId;
use rgb::validation::Validity;
use rgb::vm::{RgbIsa, WitnessOrd};
use rgb::{
    AttachId, BundleId, ContractId, DescriptorRgb, GenesisSeal, GraphSeal, Identity, OpId,
    OutputSeal, RgbDescr, RgbKeychain, RgbWallet, STATE_DATA_MAX_LEN, TransferParams, WalletError,
    WalletProvider, XChain, XOutpoint, XWitnessId,
};
use seals::SecretSeal;
use serde_crate::{Deserialize, Serialize};
use strict_types::StrictVal;
use strict_types::encoding::{FieldName, TypeName};

use crate::RgbArgs;

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
#[allow(clippy::large_enum_variant)]
pub enum Command {
    #[clap(flatten)]
    #[display(inner)]
    General(bpwallet::cli::Command),

    #[clap(flatten)]
    #[display(inner)]
    Debug(DebugCommand),

    /// Prints out list of known RGB schemata
    Schemata,
    /// Prints out list of known RGB interfaces
    Interfaces,

    /// Prints out list of known RGB contracts
    #[display("contracts")]
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
        /// Show all state, including already spent and not owned by the wallet
        #[arg(short, long)]
        all: bool,

        /// Contract identifier
        contract_id: ContractId,

        /// Interface to interpret the state data
        iface: Option<String>,
    },

    /// Print operation history for a default fungible token under a given
    /// interface
    #[display("history")]
    History {
        /// Print detailed information
        #[arg(long)]
        details: bool,

        /// Contract identifier
        contract_id: ContractId,

        /// Interface to interpret the state data
        iface: Option<String>,
    },

    /// Display all known UTXOs belonging to this wallet
    Utxos,

    /// Issues new contract
    #[display("issue")]
    Issue {
        /// Schema name to use for the contract
        schema: SchemaId, //String,

        /// Issuer identity string
        issuer: Identity,

        /// File containing contract genesis description in YAML format
        contract: PathBuf,
    },

    /// Create new invoice
    #[display("invoice")]
    Invoice {
        /// Force address-based invoice
        #[arg(short, long)]
        address_based: bool,

        /// Interface to interpret the state data
        #[arg(short, long)]
        iface: Option<String>,

        /// Operation to use for the invoice
        ///
        /// If no operation is provided, the interface default operation is used.
        #[arg(short, long)]
        operation: Option<String>,

        /// State name to use for the invoice
        ///
        /// If no state name is provided, the interface default state name for the operation is
        /// used.
        #[arg(short, long, requires = "operation")]
        assignment: Option<String>,

        /// Contract identifier
        contract_id: ContractId,

        /// State for the invoice
        state: Option<String>,
    },

    /// Prepare PSBT file for transferring RGB assets
    ///
    /// In the most of the cases you need to use `transfer` command instead of `prepare` and
    /// `consign`.
    #[display("prepare")]
    Prepare {
        /// Encode PSBT as V2
        #[clap(short = '2')]
        v2: bool,

        /// Amount of satoshis which should be paid to the address-based
        /// beneficiary
        #[arg(long, default_value = "2000")]
        sats: Sats,

        /// Invoice data
        invoice: RgbInvoice,

        /// Fee
        fee: Sats,

        /// Name of PSBT file to save. If not given, prints PSBT to STDOUT
        psbt: Option<PathBuf>,
    },

    /// Prepare consignment for transferring RGB assets
    ///
    /// In the most of the cases you need to use `transfer` command instead of `prepare` and
    /// `consign`.
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
        #[arg(short = '2')]
        v2: bool,

        /// Amount of satoshis which should be paid to the address-based
        /// beneficiary
        #[arg(long, default_value = "2000")]
        sats: Sats,

        /// Invoice data
        invoice: RgbInvoice,

        /// Fee for bitcoin transaction, in satoshis
        #[arg(short, long, default_value = "400")]
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
    /// List known tapret tweaks for a wallet
    Taprets,
}

impl Exec for RgbArgs {
    type Error = WalletError;
    const CONF_FILE_NAME: &'static str = "rgb.toml";

    fn exec(self, config: Config, _name: &'static str) -> Result<(), WalletError> {
        match &self.command {
            Command::General(cmd) => {
                self.inner.translate(cmd).exec(config, "rgb")?;
            }
            Command::Utxos => {
                self.inner
                    .translate(&BpCommand::Balance {
                        addr: true,
                        utxo: true,
                    })
                    .exec(config, "rgb")?;
            }

            Command::Debug(DebugCommand::Taprets) => {
                let stock = self.rgb_stock()?;
                for (witness_id, tapret) in stock.as_stash_provider().taprets()? {
                    println!("{witness_id}\t{tapret}");
                }
            }
            Command::Schemata => {
                let stock = self.rgb_stock()?;
                for info in stock.schemata()? {
                    print!("{info}");
                }
            }
            Command::Interfaces => {
                let stock = self.rgb_stock()?;
                for info in stock.ifaces()? {
                    print!("{info}");
                }
            }
            Command::Contracts => {
                let stock = self.rgb_stock()?;
                for info in stock.contracts()? {
                    print!("{info}");
                }
            }

            Command::History {
                contract_id,
                iface,
                details,
            } => {
                let wallet = self.rgb_wallet(&config)?;
                let iface = match contract_default_iface_name(*contract_id, wallet.stock(), iface)?
                {
                    ControlFlow::Continue(name) => name,
                    ControlFlow::Break(_) => return Ok(()),
                };
                let mut history = wallet.history(*contract_id, iface)?;
                history.sort_by_key(|op| op.witness.map(|w| w.ord).unwrap_or(WitnessOrd::Archived));
                if *details {
                    println!("Operation\tValue    \tState\t{:78}\tWitness", "Seal");
                } else {
                    println!("Operation\tValue    \t{:78}\tWitness", "Seal");
                }
                for ContractOp {
                    direction,
                    ty,
                    opids,
                    state,
                    attach_id,
                    to,
                    witness,
                } in history
                {
                    print!("{:9}\t", direction.to_string());

                    print!("{state}");
                    if *details {
                        print!("\t{ty}");
                    }
                    println!(
                        "\t{}\t{}",
                        to.first().expect("at least one receiver is always present"),
                        witness
                            .map(|info| format!("{} ({})", info.id, info.ord))
                            .unwrap_or_else(|| s!("~"))
                    );
                    if *details {
                        println!(
                            "\topid {}",
                            opids
                                .iter()
                                .map(OpId::to_string)
                                .collect::<Vec<_>>()
                                .join("\n\topid ")
                        );
                        if let Some(attach) = attach_id {
                            println!("attach {attach}");
                        }
                    }
                }
            }

            Command::Import { armored, file } => {
                let mut stock = self.rgb_stock()?;
                assert!(!armored, "importing armored files is not yet supported");
                // TODO: Support armored files
                let content = UniversalFile::load_file(file)?;
                match content {
                    UniversalFile::Kit(kit) => {
                        let id = kit.kit_id();
                        eprintln!("Importing kit {id}:");
                        let mut iface_names = map![];
                        let mut schema_names = map![];
                        for iface in &kit.ifaces {
                            let iface_id = iface.iface_id();
                            iface_names.insert(iface_id, &iface.name);
                            eprintln!("- interface {} {:-}", iface.name, iface_id);
                        }
                        for schema in &kit.schemata {
                            let schema_id = schema.schema_id();
                            schema_names.insert(schema_id, &schema.name);
                            eprintln!("- schema {} {:-}", schema.name, schema_id);
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
                            eprintln!("- implementation of {iface} for {schema}",);
                        }
                        for lib in &kit.scripts {
                            eprintln!("- script library {}", lib.id());
                        }
                        eprintln!("- strict types: {} definitions", kit.types.len());
                        let kit = kit.validate().map_err(|(status, _)| status.to_string())?;
                        stock.import_kit(kit)?;
                        eprintln!("Kit is imported");
                    }
                    UniversalFile::Contract(contract) => {
                        let id = contract.consignment_id();
                        eprintln!("Importing consignment {id}:");
                        let resolver = self.resolver()?;
                        eprint!("- validating the contract {} ... ", contract.contract_id());
                        let contract = contract
                            .validate(&resolver, self.general.network.is_testnet())
                            .map_err(|(status, _)| {
                                eprintln!("failure");
                                status.to_string()
                            })?;
                        eprintln!("success");
                        stock.import_contract(contract, &resolver)?;
                        eprintln!("Consignment is imported");
                    }
                    UniversalFile::Transfer(_) => {
                        return Err(s!("use `validate` and `accept` commands to work with \
                                       transfer consignments")
                        .into());
                    }
                }
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
            }

            Command::Armor { file } => {
                let content = UniversalFile::load_file(file)?;
                println!("{content}");
            }

            Command::State {
                contract_id,
                iface,
                all,
            } => {
                let stock_path = self.general.base_dir();
                let stock = self.load_stock(stock_path)?;

                enum StockOrWallet {
                    Stock(Stock),
                    Wallet(RgbWallet<Wallet<XpubDerivable, RgbDescr>>),
                }
                impl StockOrWallet {
                    fn stock(&self) -> &Stock {
                        match self {
                            StockOrWallet::Stock(stock) => stock,
                            StockOrWallet::Wallet(wallet) => wallet.stock(),
                        }
                    }
                }

                let iface = match contract_default_iface_name(*contract_id, &stock, iface)? {
                    ControlFlow::Continue(name) => name,
                    ControlFlow::Break(_) => return Ok(()),
                };

                let stock_wallet = match self.rgb_wallet_from_stock(&config, stock) {
                    Ok(wallet) => StockOrWallet::Wallet(wallet),
                    Err((stock, _)) => StockOrWallet::Stock(stock),
                };

                let filter = match stock_wallet {
                    StockOrWallet::Wallet(ref wallet) if *all => Filter::WalletAll(wallet),
                    StockOrWallet::Wallet(ref wallet) => Filter::Wallet(wallet),
                    StockOrWallet::Stock(_) => {
                        println!("no wallets found");
                        Filter::NoWallet
                    }
                };

                let contract = stock_wallet
                    .stock()
                    .contract_iface(*contract_id, tn!(iface.to_owned()))?;

                println!("\nGlobal:");
                for global in &contract.iface.global_state {
                    if let Ok(values) = contract.global(global.name.clone()) {
                        for val in values {
                            println!("  {} := {}", global.name, val);
                        }
                    }
                }

                enum Filter<'w> {
                    Wallet(&'w RgbWallet<Wallet<XpubDerivable, RgbDescr>>),
                    WalletAll(&'w RgbWallet<Wallet<XpubDerivable, RgbDescr>>),
                    NoWallet,
                }
                impl<'w> AssignmentsFilter for Filter<'w> {
                    fn should_include(
                        &self,
                        outpoint: impl Into<XOutpoint>,
                        id: Option<XWitnessId>,
                    ) -> bool {
                        match self {
                            Filter::Wallet(wallet) => wallet
                                .wallet()
                                .filter_unspent()
                                .should_include(outpoint, id),
                            _ => true,
                        }
                    }
                }
                impl<'w> Filter<'w> {
                    fn comment(&self, outpoint: XOutpoint) -> &'static str {
                        let outpoint = outpoint
                            .into_bp()
                            .into_bitcoin()
                            .expect("liquid is not yet supported");
                        match self {
                            Filter::Wallet(rgb) if rgb.wallet().is_unspent(outpoint) => "",
                            Filter::WalletAll(rgb) if rgb.wallet().is_unspent(outpoint) => {
                                "-- unspent"
                            }
                            Filter::WalletAll(rgb) if rgb.wallet().has_outpoint(outpoint) => {
                                "-- spent"
                            }
                            _ => "-- third-party",
                        }
                    }
                }

                println!("\nOwned:");
                for owned in &contract.iface.assignments {
                    println!("  State      \t{:78}\tWitness", "Seal");
                    println!("  {}:", owned.name);
                    if let Ok(outputs) = contract.outputs_by_type(owned.name.clone(), &filter) {
                        for output in outputs {
                            let witness = output
                                .witness
                                .and_then(|w| contract.witness_info(w))
                                .map(|info| format!("{} ({})", info.id, info.ord))
                                .unwrap_or_else(|| s!("~"));
                            println!(
                                "    {: >9}\t{}\t{} {}",
                                output.state,
                                output.seal,
                                witness,
                                filter.comment(output.seal.to_outpoint())
                            );
                        }
                    }
                }
            }
            Command::Issue {
                schema: schema_id,
                issuer,
                contract,
            } => {
                let mut stock = self.rgb_stock()?;
                let file = File::open(contract)?;
                let src = serde_yaml::from_reader::<_, serde_yaml::Value>(file)?;
                let code = src.as_mapping().expect("invalid YAML root-level structure");

                let iface_name = code
                    .get("interface")
                    .expect("contract must specify interface under which it is constructed")
                    .as_str()
                    .expect("interface name must be a string");
                let iface_name = tn!(iface_name.to_owned());
                let iface = stock
                    .iface(iface_name.clone())
                    .or_else(|_| {
                        let id = IfaceId::from_str(iface_name.as_str())?;
                        stock.iface(id).map_err(WalletError::from)
                    })?
                    .clone();
                let iface_id = iface.iface_id();

                let mut builder = stock.contract_builder(issuer.clone(), *schema_id, iface_id)?;

                if let Some(globals) = code.get("globals") {
                    for (name, val) in globals
                        .as_mapping()
                        .expect("invalid YAML: globals must be an mapping")
                    {
                        let name = name
                            .as_str()
                            .expect("invalid YAML: global name must be a string");
                        // Workaround for borrow checker:
                        let field_name =
                            FieldName::try_from(name.to_owned()).expect("invalid type name");
                        let value = StrictVal::from(val.clone());
                        builder = builder
                            .add_global_state(field_name, value)
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
                        // Workaround for borrow checker:
                        let field_name =
                            FieldName::try_from(name.to_owned()).expect("invalid type name");

                        let assign = val.as_mapping().expect("an assignment must be a mapping");
                        let seal = assign
                            .get("seal")
                            .expect("assignment doesn't provide seal information")
                            .as_str()
                            .expect("seal must be a string");
                        let seal = OutputSeal::from_str(seal).expect("invalid seal definition");
                        let seal = GenesisSeal::new_random(seal.method, seal.txid, seal.vout);
                        let seal = BuilderSeal::Revealed(XChain::Bitcoin(seal));

                        let data =
                            StrictVal::from(assign.get("state").expect("absent state").clone());
                        let attach = assign.get("attachment").map(|id| {
                            AttachId::from_str(id.as_str().expect("invalid attachment data"))
                                .expect("invalid attachment id string")
                        });
                        builder = builder
                            .add_owned_state(field_name, seal, data, attach)
                            .expect("invalid owned state data");
                    }
                }

                let contract = builder.issue_contract()?;
                let id = contract.contract_id();
                stock.import_contract(contract, &ContractIssueResolver)?;
                eprintln!(
                    "A new contract {id} is issued and added to the stash.\nUse `export` command \
                     to export the contract."
                );
            }
            Command::Invoice {
                address_based,
                operation,
                assignment,
                contract_id,
                iface,
                state,
            } => {
                let mut wallet = self.rgb_wallet(&config)?;

                let outpoint = wallet
                    .wallet()
                    .coinselect(Sats::ZERO, |utxo| {
                        RgbKeychain::contains_rgb(utxo.terminal.keychain)
                    })
                    .next();
                let network = wallet.wallet().network();
                let beneficiary = match (address_based, outpoint) {
                    (false, None) => {
                        return Err(WalletError::Custom(s!(
                            "blinded invoice requested but no suitable outpoint is available"
                        )));
                    }
                    (true, _) => {
                        let addr = wallet
                            .wallet()
                            .addresses(RgbKeychain::Rgb)
                            .next()
                            .expect("no addresses left")
                            .addr;
                        Beneficiary::WitnessVout(Pay2Vout {
                            address: addr.payload,
                            method: wallet.wallet().seal_close_method(),
                        })
                    }
                    (_, Some(outpoint)) => {
                        let seal = XChain::Bitcoin(GraphSeal::new_random(
                            wallet.wallet().seal_close_method(),
                            outpoint.txid,
                            outpoint.vout,
                        ));
                        wallet.stock_mut().store_secret_seal(seal)?;
                        Beneficiary::BlindedSeal(*seal.to_secret_seal().as_reduced_unsafe())
                    }
                };

                let iface = match contract_default_iface_name(*contract_id, wallet.stock(), iface)?
                {
                    ControlFlow::Continue(name) => wallet.stock().iface(name)?,
                    ControlFlow::Break(_) => return Ok(()),
                };
                let iface_name = &iface.name;
                let Some(op_name) = operation
                    .clone()
                    .map(FieldName::try_from)
                    .transpose()
                    .map_err(|e| WalletError::Invoicing(format!("invalid operation name - {e}")))?
                    .or(iface.default_operation.clone())
                else {
                    return Err(WalletError::Invoicing(format!(
                        "interface {iface_name} doesn't have default operation"
                    )));
                };
                let Some(iface_op) = iface.transitions.get(&op_name) else {
                    return Err(WalletError::Invoicing(format!(
                        "interface {iface_name} doesn't have operation {op_name}"
                    )));
                };
                let state_name = assignment
                    .clone()
                    .map(FieldName::try_from)
                    .transpose()
                    .map_err(|e| WalletError::Invoicing(format!("invalid state name - {e}")))?
                    .or_else(|| iface_op.default_assignment.clone())
                    .ok_or_else(|| {
                        WalletError::Invoicing(format!(
                            "interface {iface_name} doesn't have a default state for the \
                             operation {op_name}"
                        ))
                    })?;
                let Some(assign_iface) = iface.assignments.get(&state_name) else {
                    return Err(WalletError::Invoicing(format!(
                        "interface {iface_name} doesn't have state {state_name} in operation \
                         {op_name}"
                    )));
                };

                let mut builder = RgbInvoiceBuilder::new(XChainNet::bitcoin(network, beneficiary))
                    .set_contract(*contract_id)
                    .set_interface(iface_name.clone());

                if operation.is_some() {
                    builder = builder.set_operation(op_name);
                    if let Some(state) = assignment {
                        builder = builder.set_operation(fname!(state.clone()));
                    }
                }

                if let Some(state) = state {
                    let state: StrictVal = serde_yaml::from_str(&state)?;
                    let Some(sem_id) = assign_iface.state_ty else {
                        return Err(WalletError::Invoicing(format!(
                            "interface {iface_name} doesn't define a state type for the invoiced \
                             assignment"
                        )));
                    };
                    let types = wallet.stock().type_system(iface)?;
                    let value = types
                        .typify(state, sem_id)
                        .map_err(|e| WalletError::Invoicing(format!("invalid state data - {e}")))?;
                    let data = types
                        .strict_serialize_value::<STATE_DATA_MAX_LEN>(&value)
                        .map_err(|_| WalletError::Invoicing(s!("state data too large")))?;
                    builder = builder.set_state(data.into());
                }

                let invoice = builder.finish();
                println!("{invoice}");
            }
            Command::Prepare {
                v2,
                invoice,
                fee,
                sats,
                psbt: psbt_file,
            } => {
                let mut wallet = self.rgb_wallet(&config)?;
                // TODO: Support lock time and RBFs
                let params = TransferParams::with(*fee, *sats);

                let (psbt, _) = wallet
                    .construct_psbt(invoice.clone(), params)
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
            }
            Command::Consign {
                invoice,
                psbt: psbt_name,
                consignment: out_file,
            } => {
                let mut wallet = self.rgb_wallet(&config)?;
                let mut psbt_file = File::open(psbt_name)?;
                let mut psbt = Psbt::decode(&mut psbt_file)?;
                let transfer = wallet
                    .transfer(invoice, &mut psbt)
                    .map_err(|err| err.to_string())?;
                let mut psbt_file = File::create(psbt_name)?;
                psbt.encode(psbt.version, &mut psbt_file)?;
                transfer.save_file(out_file)?;
            }
            Command::Transfer {
                v2,
                invoice,
                fee,
                sats,
                psbt: psbt_file,
                consignment: out_file,
            } => {
                let mut wallet = self.rgb_wallet(&config)?;
                // TODO: Support lock time and RBFs
                let params = TransferParams::with(*fee, *sats);

                let (mut psbt, _, transfer) =
                    wallet.pay(invoice, params).map_err(|err| err.to_string())?;

                transfer.save_file(out_file)?;

                psbt.version = if *v2 { PsbtVer::V2 } else { PsbtVer::V0 };
                match psbt_file {
                    Some(file_name) => {
                        let mut psbt_file = File::create(file_name)?;
                        psbt.encode(psbt.version, &mut psbt_file)?;
                    }
                    None => println!("{psbt}"),
                }
            }
            Command::Inspect { file, dir, path } => {
                #[derive(Clone, Debug)]
                #[derive(Serialize, Deserialize)]
                #[serde(crate = "serde_crate", rename_all = "camelCase")]
                pub struct ConsignmentInspection {
                    version: ContainerVer,
                    transfer: bool,
                    terminals: SmallOrdMap<BundleId, XChain<SecretSeal>>,
                    supplements: TinyOrdSet<Supplement>,
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
                        lib.print_disassemble::<RgbIsa<MemContract>>(&mut buf)?;
                        map.insert(format!("{}.aluasm", lib.id().to_baid64_mnemonic()), unsafe {
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
                fs::create_dir_all(format!("{root_dir}/stash/supplements"))?;
                fs::create_dir_all(format!("{root_dir}/state"))?;
                fs::create_dir_all(format!("{root_dir}/index"))?;

                // Stash
                for (id, schema_ifaces) in stock.as_stash_provider().debug_schemata() {
                    fs::write(
                        format!(
                            "{root_dir}/stash/schemata/{}.{id:-#}.yaml",
                            schema_ifaces.schema.name
                        ),
                        serde_yaml::to_string(&schema_ifaces)?,
                    )?;
                }
                for (id, iface) in stock.as_stash_provider().debug_ifaces() {
                    fs::write(
                        format!("{root_dir}/stash/ifaces/{}.{id:-#}.yaml", iface.name),
                        serde_yaml::to_string(stock.iface(*id)?)?,
                    )?;
                }
                for (id, genesis) in stock.as_stash_provider().debug_geneses() {
                    fs::write(
                        format!("{root_dir}/stash/geneses/{id:-}.yaml"),
                        serde_yaml::to_string(genesis)?,
                    )?;
                }
                for (id, list) in stock.as_stash_provider().debug_suppl() {
                    for suppl in list {
                        fs::write(
                            format!(
                                "{root_dir}/stash/geneses/{id:-}.suppl.{}.yaml",
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
                for (id, suppl) in stock.as_stash_provider().debug_suppl() {
                    fs::write(
                        format!("{root_dir}/stash/supplements/{id:#}.yaml"),
                        serde_yaml::to_string(suppl)?,
                    )?;
                }
                fs::write(
                    format!("{root_dir}/stash/seal-secret.yaml"),
                    serde_yaml::to_string(stock.as_stash_provider().debug_secret_seals())?,
                )?;
                // TODO: Add sigs debugging

                // State
                fs::write(
                    format!("{root_dir}/state/witnesses.yaml"),
                    serde_yaml::to_string(stock.as_state_provider().debug_witnesses())?,
                )?;
                for (id, state) in stock.as_state_provider().debug_contracts() {
                    fs::write(
                        format!("{root_dir}/state/{id:-}.yaml"),
                        serde_yaml::to_string(state)?,
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
            }
            Command::Validate { file } => {
                let mut resolver = self.resolver()?;
                let consignment = Transfer::load_file(file)?;
                resolver.add_terminals(&consignment);
                let status =
                    match consignment.validate(&resolver, self.general.network.is_testnet()) {
                        Ok(consignment) => consignment.into_validation_status(),
                        Err((status, _)) => status,
                    };
                if status.validity() == Validity::Valid {
                    eprintln!("The provided consignment is valid")
                } else {
                    eprintln!("{status}");
                }
            }
            Command::Accept { force: _, file } => {
                // TODO: Ensure we properly handle unmined terminal transactions
                let mut stock = self.rgb_stock()?;
                let mut resolver = self.resolver()?;
                let transfer = Transfer::load_file(file)?;
                resolver.add_terminals(&transfer);
                let valid = transfer
                    .validate(&resolver, self.general.network.is_testnet())
                    .map_err(|(status, _)| status)?;
                stock.accept_transfer(valid, &resolver)?;
                eprintln!("Transfer accepted into the stash");
            }
        }
        Ok(())
    }
}

fn contract_default_iface_name(
    contract_id: ContractId,
    stock: &Stock,
    iface: &Option<String>,
) -> Result<ControlFlow<(), TypeName>, StockError> {
    if let Some(iface) = iface {
        return Ok(ControlFlow::Continue(tn!(iface.clone())));
    };
    let info = stock.contract_info(contract_id)?;
    let schema = stock.schema(info.schema_id)?;
    Ok(match schema.iimpls.len() {
        0 => {
            eprintln!("contract doesn't implement any interface and thus can't be read");
            ControlFlow::Break(())
        }
        1 => ControlFlow::Continue(
            schema
                .iimpls
                .first_key_value()
                .expect("one interface is present")
                .0
                .clone(),
        ),
        _ => {
            eprintln!(
                "contract implements multiple interface, please select one of them to read the \
                 contract:"
            );
            for iface in schema.iimpls.keys() {
                eprintln!("{iface}");
            }
            ControlFlow::Break(())
        }
    })
}
