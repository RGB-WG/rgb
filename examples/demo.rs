// Standard Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
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

#[cfg(feature = "fs")]
use crate::fs::run_file_resolver_example;

#[cfg(feature = "fs")]
mod fs {
    use std::convert::Infallible;
    use std::fs::{create_dir, read_to_string, remove_dir_all, File};
    use std::io::Write;
    use std::num::NonZeroU64;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    use amplify::none;
    use bpstd::psbt::{ChangeInfo, Output, TxParams, Utxo};
    use bpstd::{
        IdxBase, Network, Outpoint, PsbtVer, Sats, ScriptPubkey, SeqNo, SigScript, Terminal, Tx,
        TxIn, Txid, UnsignedTx, VarIntArray,
    };
    use indexmap::IndexMap;
    use rgb::invoice::{RgbBeneficiary, RgbInvoice};
    use rgb::popls::bp::WalletProvider;
    use rgb::{Consensus, Contracts, CreateParams, Issuer, StateArithm, WitnessStatus};
    use rgb_persist_fs::StockpileDir;
    use rgbp::resolvers::{Resolver, ResolverError};
    use rgbp::{CoinselectStrategy, FileHolder, Owner, RgbpRuntimeDir};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, PartialEq, Eq, Debug, Default)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "camelCase"))]
    struct FileIndexer {
        last_block: u64,
        mined: IndexMap<String, u64>,
        txs: IndexMap<String, String>,
        utxos: IndexMap<Outpoint, (Sats, Terminal)>,
    }

    #[derive(Clone, PartialEq, Eq, Debug, Default)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(rename_all = "camelCase"))]
    pub struct FileResolver {
        path: PathBuf,
    }

    impl FileResolver {
        pub fn load(path: PathBuf) -> Self { Self { path } }
    }

    impl Resolver for FileResolver {
        fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
            let file = read_to_string(&self.path).expect("cannot load file");
            let model: FileIndexer = toml::from_str(&file).expect("invalid last block");

            model
                .txs
                .get(&txid.to_string())
                .map_or(Ok(None), |tx| Ok(Some(Tx::from_str(tx).unwrap().into())))
        }

        fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
            let file = read_to_string(&self.path).expect("cannot load file");
            let model: FileIndexer = toml::from_str(&file).expect("invalid last block");

            model
                .mined
                .get(&txid.to_string())
                .map_or(Ok(WitnessStatus::Tentative), |tip| {
                    Ok(WitnessStatus::Mined(NonZeroU64::new(*tip).unwrap()))
                })
        }

        fn resolve_utxos(
            &self,
            _iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
        ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
            let file = read_to_string(&self.path).expect("cannot load file");
            let model: FileIndexer = toml::from_str(&file).expect("invalid last block");

            model
                .utxos
                .clone()
                .into_iter()
                .map(|(outpoint, (value, terminal))| Ok(Utxo { outpoint, value, terminal }))
        }

        fn last_block_height(&self) -> Result<u64, ResolverError> {
            let file = read_to_string(&self.path).expect("cannot load file");
            let model: FileIndexer = toml::from_str(&file).expect("invalid last block");

            Ok(model.last_block)
        }

        fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
            let file = read_to_string(&self.path).expect("cannot load file");
            let mut model: FileIndexer = toml::from_str(&file).expect("invalid last block");

            let tx_id = tx.txid();

            model.mined.insert(tx_id.to_string(), model.last_block);
            model.txs.insert(tx.txid().to_string(), tx.to_string());

            model.last_block += 1;

            let mut file = File::create(&self.path).expect("cannot create new file");
            let content = toml::to_string(&model).expect("cannot parse data");
            file.write_all(content.as_bytes())
                .expect("cannot be write file");

            Ok(())
        }
    }

    pub fn create_rgb_wallet(label: &str) -> RgbpRuntimeDir<FileResolver> {
        let network = Network::Testnet4;
        let wallet_path = "./examples/data/bitcoin.testnet";
        let rgb_path = "./examples/data/rgb.contracts";
        let issuers_path = "./examples/data/rgb.issuers";

        let label_path = Path::new(wallet_path).join(format!("{label}.wallet"));
        let label_stock_path = Path::new(rgb_path).join(format!("{label}.stock"));
        remove_dir_all(&label_stock_path).expect("cannot exclude dir");
        create_dir(&label_stock_path).expect("cannot create dir");

        let indexer_path = Path::new(wallet_path)
            .join(format!("{label}.wallet"))
            .join("indexer.toml");
        let resolver = FileResolver::load(indexer_path);

        let stockpile = StockpileDir::load(label_stock_path, Consensus::Bitcoin, true)
            .expect("Invalid contracts directory");
        let demo_contracts = Contracts::load(stockpile);

        let hodler = FileHolder::load(label_path.clone()).unwrap_or_else(|err| {
            panic!(
                "unable to load wallet from path `{}`\nDetails: {err}",
                label_path.to_str().unwrap_or_default()
            )
        });

        let owner = Owner::with_components(network, hodler, resolver);
        let mut runtime = RgbpRuntimeDir::with_components(owner, demo_contracts);

        let issuer_import_path = Path::new(issuers_path).join("RGB20-FNA.issuer");
        let issuer = Issuer::load(issuer_import_path, |_, _, _| Result::<_, Infallible>::Ok(()))
            .expect("Unable to load issue file");
        runtime
            .contracts
            .import_issuer(issuer)
            .expect("Unable to process import issuer definitions");
        runtime
    }

    pub fn run_file_resolver_example() {
        let network = Network::Testnet3;

        let rgb_backup_path = "./examples/data/rgb.contracts";
        let rgb_issuers_path = "./examples/data/rgb.issuers";

        let params_path = Path::new(rgb_issuers_path).join("IssueCmd.yaml");
        let params_file = File::open(params_path).expect("Unable to open the parameter file");
        let params = serde_yaml::from_reader::<_, CreateParams<Outpoint>>(params_file)
            .expect("Unable to process issue parameters");

        // 1. Issue new contract
        let mut alice = create_rgb_wallet("alice");
        alice.issue(params).expect("cannot create a new issue");

        // 2. Consume/Import Contract
        let contract_id = alice.contracts.contract_ids().next().unwrap();

        let alice_backup = Path::new(rgb_backup_path).join("alice.stock/backup.contract");
        alice
            .contracts
            .export_to_file(alice_backup.clone(), contract_id)
            .expect("cannot create backup contract");

        let mut bob = create_rgb_wallet("bob");
        bob.consume_from_file(true, alice_backup, |_, _, _| Result::<_, Infallible>::Ok(()))
            .expect("cannot import/accept contract");

        // 3. Check Contract States
        let alice_state = alice.wallet_contract_state(contract_id);
        let bob_state = bob.wallet_contract_state(contract_id);

        let mut alice_calc = StateArithm::Fungible.calculator();
        alice_state.owned.iter().for_each(|(_, states)| {
            states
                .iter()
                .for_each(|x| alice_calc.accumulate(&x.assignment.data).expect(""));
        });

        let mut bob_calc = StateArithm::Fungible.calculator();
        bob_state.owned.iter().for_each(|(_, states)| {
            states
                .iter()
                .for_each(|x| bob_calc.accumulate(&x.assignment.data).expect(""));
        });

        eprintln!(":: Before Transfer ::");
        eprintln!("Alice: {:?}", alice_calc);
        eprintln!("Bob: {:?}", bob_calc);

        // 4. Create new Invoice/Payment Script
        let auth = bob.auth_token(none!()).expect("cannot create auth token");
        let beneficiary = RgbBeneficiary::Token(auth);

        let bob_invoice = RgbInvoice::new(
            contract_id,
            Consensus::Bitcoin,
            network.is_testnet(),
            beneficiary,
            Some(500u64.into()),
        );

        // 5. Create new Transfer/Consignment
        let (psbt, payment) = alice
            .pay_invoice(
                &bob_invoice,
                CoinselectStrategy::SmallSize,
                TxParams::with(144u64.into()),
                none!(),
            )
            .expect("cannot be process payment");

        let alice_psbt_path = Path::new(rgb_backup_path).join("alice.stock/transfer.psbt");
        let alice_consig_path = Path::new(rgb_backup_path).join("alice.stock/transfer.rgb");

        let mut psbt_file = File::create(alice_psbt_path).expect("cannot create psbt file");
        psbt.encode(PsbtVer::V0, &mut psbt_file)
            .expect("cannot encode psbt");

        alice
            .contracts
            .consign_to_file(alice_consig_path.clone(), bob_invoice.scope, payment.terminals)
            .expect("cannot create consig file");

        // 6. Publish L1 TX
        let ChangeInfo { vout, terminal, .. } = payment.psbt_meta.change.unwrap();
        let tx = Tx {
            version: psbt.tx_version,
            inputs: VarIntArray::from_iter_checked(psbt.inputs().map(|txin| TxIn {
                prev_output: txin.previous_outpoint,
                sig_script: SigScript::empty(),
                sequence: txin.sequence_number.unwrap_or(SeqNo::from_consensus_u32(0)),
                witness: txin.final_witness.clone().unwrap_or_default(),
            })),
            outputs: VarIntArray::from_iter_checked(psbt.outputs().map(Output::to_txout)),
            lock_time: psbt.lock_time(),
        };

        // 6. Sync Wallets
        // alice.update(1).expect("unable update wallet");
        alice
            .wallet
            .broadcast(&tx, Some((vout, terminal.keychain.index(), terminal.index.index())))
            .expect("cannot be broadcast");
        bob.update(1).expect("unable update wallet");

        // 7. Accept new Transfer/Consignment
        alice
            .consume_from_file(true, alice_consig_path.clone(), |_, _, _| {
                Result::<_, Infallible>::Ok(())
            })
            .expect("cannot accept transfer");
        bob.consume_from_file(true, alice_consig_path, |_, _, _| Result::<_, Infallible>::Ok(()))
            .expect("cannot accept transfer");

        // 7. Check Contract States
        let alice_state = alice.wallet_contract_state(contract_id);
        let bob_state = bob.wallet_contract_state(contract_id);

        let mut alice_calc = StateArithm::Fungible.calculator();
        alice_state.owned.iter().for_each(|(_, states)| {
            states
                .iter()
                .for_each(|x| alice_calc.accumulate(&x.assignment.data).expect(""));
        });

        let mut bob_calc = StateArithm::Fungible.calculator();
        bob_state.owned.iter().for_each(|(_, states)| {
            states
                .iter()
                .for_each(|x| bob_calc.accumulate(&x.assignment.data).expect(""));
        });

        eprintln!("\n:: After Transfer ::");
        eprintln!("Alice: {:?}", alice_calc);
        eprintln!("Bob: {:?}", bob_calc);
    }
}

fn main() {
    #[cfg(feature = "fs")]
    run_file_resolver_example();
}
