use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use bpstd::psbt::{PsbtConstructor, TxParams};
use bpstd::signers::TestnetSigner;
use bpstd::{
    h, Address, HardenedIndex, Keychain, Network, Psbt, Sats, Tx, Txid, Vout, Wpkh, XprivAccount,
    XpubDerivable,
};
use bpwallet::fs::FsTextStore;
use bpwallet::AnyIndexer;
use rand::RngCore;
use rgb::invoice::{RgbBeneficiary, RgbInvoice};
use rgb::popls::bp::file::{BpDirMound, DirBarrow};
use rgb::{
    Assignment, CodexId, Consensus, ContractId, CreateParams, EitherSeal, NamedState, Outpoint,
    StateAtom,
};
use rgbp::descriptor::RgbDescr;
use rgbp::{CoinselectStrategy, RgbDirRuntime, RgbWallet};
use strict_types::{svenum, svnum, svstr, tn, vname, StrictVal};

use crate::utils::chain::{
    broadcast_tx, fund_wallet, get_indexer, indexer_url, is_tx_mined, mine_custom, INSTANCE_1,
};
use crate::utils::report::Report;
use crate::utils::{AssetSchema, DescriptorType, DEFAULT_FEE_ABS};

pub struct TestRuntime {
    rt: RgbDirRuntime,
    signer: TestnetSigner,
    instance: u8,
}

impl Deref for TestRuntime {
    type Target = RgbDirRuntime;
    fn deref(&self) -> &Self::Target { &self.rt }
}
impl DerefMut for TestRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.rt }
}

impl TestRuntime {
    pub fn new(descriptor_type: &DescriptorType) -> Self { Self::with(descriptor_type, INSTANCE_1) }

    pub fn with(descriptor_type: &DescriptorType, instance: u8) -> Self {
        let mut seed = vec![0u8; 128];
        rand::rng().fill_bytes(&mut seed);

        let xpriv_account = XprivAccount::with_seed(true, &seed).derive(h![86, 1, 0]);

        let fingerprint = xpriv_account.account_fp().to_string();
        let wallet_dir = PathBuf::from("tests").join("test-data").join(fingerprint);

        Self::_init(descriptor_type, wallet_dir, xpriv_account, instance)
    }

    fn _init(
        descriptor_type: &DescriptorType,
        wallet_dir: PathBuf,
        account: XprivAccount,
        instance: u8,
    ) -> Self {
        std::fs::create_dir_all(&wallet_dir).unwrap();
        println!("wallet dir: {wallet_dir:?}");

        let xpub = account.to_xpub_account();
        let xpub = XpubDerivable::with(xpub, &[Keychain::OUTER, Keychain::INNER]);
        let signer = TestnetSigner::new(account);

        let mut mound = BpDirMound::load_testnet(Consensus::Bitcoin, &wallet_dir, true);
        mound
            .load_issuer("tests/fixtures/NonInflatableAsset.issuer")
            .unwrap();
        mound
            .load_issuer("tests/fixtures/CollectibleFungibleAsset.issuer")
            .unwrap();

        let provider = FsTextStore::new(wallet_dir).expect("Broken directory structure");
        let noise = xpub.xpub().chain_code().to_byte_array();
        let descr = match descriptor_type {
            DescriptorType::Wpkh => RgbDescr::new_unfunded(Wpkh::from(xpub), noise),
            DescriptorType::Tr => RgbDescr::key_only_unfunded(xpub, noise),
        };
        let wallet = RgbWallet::create(provider, descr, Network::Regtest, true).unwrap();
        let rt = RgbDirRuntime::from(DirBarrow::with(wallet, mound));

        let mut me = Self { rt, signer, instance };
        me.sync();
        me
    }

    pub fn get_address(&self) -> Address {
        self.wallet
            .addresses(Keychain::OUTER)
            .next()
            .expect("no addresses left")
            .addr
    }

    pub fn get_utxo(&mut self, sats: Option<u64>) -> Outpoint {
        let address = self.get_address();
        let txid = Txid::from_str(&fund_wallet(address.to_string(), sats, self.instance)).unwrap();
        self.sync();
        let mut vout = None;
        let coins = self.wallet.address_coins();
        assert!(!coins.is_empty());
        for (_derived_addr, utxos) in coins {
            for utxo in utxos {
                if utxo.outpoint.txid == txid {
                    vout = Some(utxo.outpoint.vout_u32());
                }
            }
        }
        Outpoint { txid, vout: Vout::from_u32(vout.unwrap()) }
    }

    pub fn issue_nia(&mut self, name: &str, issued_supply: u64, outpoint: Outpoint) -> ContractId {
        let params = CreateParams {
            codex_id: CodexId::from_str(
                "qaeakTdk-FccgZC9-4yYpoHa-quPSbQL-XmyBxtn-2CpD~38#jackson-couple-oberon",
            )
            .unwrap(),
            consensus: Consensus::Bitcoin,
            testnet: true,
            method: vname!("issue"),
            name: tn!("NIA"),
            timestamp: None,
            global: vec![
                // TODO: simplify API for named state creation
                NamedState {
                    name: vname!("name"),
                    state: StateAtom { verified: svstr!(name), unverified: None },
                },
                NamedState {
                    name: vname!("ticker"),
                    state: StateAtom { verified: svstr!("NIA"), unverified: None },
                },
                NamedState {
                    name: vname!("precision"),
                    state: StateAtom { verified: svenum!(centiMilli), unverified: None },
                },
                NamedState {
                    name: vname!("circulating"),
                    state: StateAtom { verified: svnum!(issued_supply), unverified: None },
                },
            ],
            owned: vec![NamedState {
                name: vname!("owned"),
                state: Assignment { seal: EitherSeal::Alt(outpoint), data: svnum!(issued_supply) },
            }],
        };
        self.rt.issue_to_file(params).unwrap()
    }

    pub fn issue_cfa(&mut self, name: &str, issued_supply: u64, outpoint: Outpoint) -> ContractId {
        let params = CreateParams {
            codex_id: CodexId::from_str(
                "6bl9LdZ_-BU8Skh9-f~4UazR-TFwyotq-ac4yebi-zodXJnw#weather-motif-patriot",
            )
            .unwrap(),
            consensus: Consensus::Bitcoin,
            testnet: true,
            method: vname!("issue"),
            name: tn!("CFA"),
            timestamp: None,
            global: vec![
                // TODO: simplify API for named state creation
                NamedState {
                    name: vname!("name"),
                    state: StateAtom { verified: svstr!(name), unverified: None },
                },
                NamedState {
                    name: vname!("details"),
                    state: StateAtom {
                        verified: StrictVal::Unit,
                        unverified: Some(svstr!("Demo CFA asset")),
                    },
                },
                NamedState {
                    name: vname!("precision"),
                    state: StateAtom { verified: svenum!(centiMilli), unverified: None },
                },
                NamedState {
                    name: vname!("circulating"),
                    state: StateAtom { verified: svnum!(issued_supply), unverified: None },
                },
            ],
            owned: vec![NamedState {
                name: vname!("owned"),
                state: Assignment { seal: EitherSeal::Alt(outpoint), data: svnum!(issued_supply) },
            }],
        };
        self.rt.issue_to_file(params).unwrap()
    }

    pub fn invoice(
        &mut self,
        contract_id: ContractId,
        amount: u64,
        wout: bool,
    ) -> RgbInvoice<ContractId> {
        let beneficiary = if wout {
            let wout = self.rt.wout(None);
            RgbBeneficiary::WitnessOut(wout)
        } else {
            let auth = self.rt.auth_token(None).unwrap();
            RgbBeneficiary::Token(auth)
        };
        let value = StrictVal::num(amount);
        RgbInvoice::new(contract_id, beneficiary, Some(value))
    }

    pub fn send(
        &mut self,
        recv_wlt: &mut TestRuntime,
        wout: bool,
        contract_id: ContractId,
        amount: u64,
        sats: u64,
        report: Option<&Report>,
    ) -> (PathBuf, Tx) {
        let invoice = recv_wlt.invoice(contract_id, amount, wout);
        self.send_to_invoice(recv_wlt, invoice, Some(sats), None, report)
    }

    pub fn send_to_invoice(
        &mut self,
        recv_wlt: &mut TestRuntime,
        invoice: RgbInvoice<ContractId>,
        sats: Option<u64>,
        fee: Option<u64>,
        report: Option<&Report>,
    ) -> (PathBuf, Tx) {
        let (consignment, tx) = self.transfer(invoice, sats, fee, true, report);
        self.mine_tx(tx.txid(), false);
        recv_wlt.accept_transfer(&consignment, report);
        self.sync();
        (consignment, tx)
    }

    pub fn transfer(
        &mut self,
        invoice: RgbInvoice<ContractId>,
        sats: Option<u64>,
        fee: Option<u64>,
        broadcast: bool,
        report: Option<&Report>,
    ) -> (PathBuf, Tx) {
        static COUNTER: OnceLock<AtomicU32> = OnceLock::new();

        let counter = COUNTER.get_or_init(|| AtomicU32::new(0));
        counter.fetch_add(1, Ordering::SeqCst);
        let consignment_no = counter.load(Ordering::SeqCst);

        self.sync();

        let fee = Sats::from_sats(fee.unwrap_or(DEFAULT_FEE_ABS));
        let sats = Sats::from_sats(sats.unwrap_or(2000));
        let strategy = CoinselectStrategy::Aggregate;
        let pay_start = Instant::now();
        let params = TxParams::with(fee);
        let (mut psbt, terminal) = self
            .pay_invoice(&invoice, strategy, params, Some(sats))
            .unwrap();

        let pay_duration = pay_start.elapsed();
        if let Some(report) = report {
            report.write_duration(pay_duration);
        }

        let tx = self.sign_finalize_extract(&mut psbt);

        println!("transfer txid: {}, consignment: {consignment_no}", tx.txid());

        if broadcast {
            self.broadcast_tx(&tx);
        }

        let consignment = PathBuf::new()
            .join("tests")
            .join("test-data")
            .join(format!("consignment-{consignment_no}"))
            .with_extension("rgb");
        self.mound
            .consign_to_file(invoice.scope, [terminal], &consignment)
            .unwrap();

        (consignment, tx)
    }

    pub fn accept_transfer(&mut self, consignment: &Path, report: Option<&Report>) {
        self.sync();
        let accept_start = Instant::now();
        self.consume_from_file(consignment).unwrap();
        let accept_duration = accept_start.elapsed();
        if let Some(report) = report {
            report.write_duration(accept_duration);
        }
    }

    pub fn check_allocations(
        &mut self,
        contract_id: ContractId,
        asset_schema: AssetSchema,
        mut expected_fungible_allocations: Vec<u64>,
        nonfungible_allocation: bool,
    ) {
        match asset_schema {
            AssetSchema::Nia | AssetSchema::Cfa => {
                let state = self.rt.state_own(Some(contract_id)).next().unwrap().1;
                let mut actual_fungible_allocations = state
                    .owned
                    .get("owned")
                    .unwrap()
                    .iter()
                    .map(|(_, assignment)| assignment.data.unwrap_num().unwrap_uint::<u64>())
                    .collect::<Vec<_>>();
                actual_fungible_allocations.sort();
                expected_fungible_allocations.sort();
                assert_eq!(actual_fungible_allocations, expected_fungible_allocations);
            }
            AssetSchema::Uda => {
                todo!()
            }
        }
    }
    pub fn sync(&mut self) {
        let indexer = self.get_indexer();
        self.wallet.update(&indexer).into_result().unwrap();
    }

    pub fn network(&self) -> Network { self.wallet.network() }

    fn get_indexer(&self) -> AnyIndexer { get_indexer(&self.indexer_url()) }

    pub fn indexer_url(&self) -> String { indexer_url(self.instance, self.network()) }

    pub fn sign_finalize(&self, psbt: &mut Psbt) {
        let _sig_count = psbt.sign(&self.signer).unwrap();
        psbt.finalize(self.wallet.descriptor());
    }

    pub fn sign_finalize_extract(&self, psbt: &mut Psbt) -> Tx {
        self.sign_finalize(psbt);
        psbt.extract().unwrap()
    }

    pub fn mine_tx(&self, txid: Txid, resume: bool) {
        let mut attempts = 10;
        loop {
            mine_custom(resume, self.instance, 1);
            if is_tx_mined(txid, &self.get_indexer()) {
                break;
            }
            attempts -= 1;
            if attempts == 0 {
                panic!("TX is not getting mined");
            }
        }
    }

    pub fn broadcast_tx(&self, tx: &Tx) { broadcast_tx(tx, &self.indexer_url()); }
}
