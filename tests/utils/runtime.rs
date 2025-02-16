use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use bpstd::psbt::{PsbtConstructor, TxParams};
use bpstd::signers::TestnetSigner;
use bpstd::{Network, Psbt, Sats, Tx};
use bpwallet::AnyIndexer;
use rgb::invoice::RgbInvoice;
use rgb::ContractId;
use rgbp::{CoinselectStrategy, RgbDirRuntime};

use crate::utils::chain::{broadcast_tx, get_indexer, indexer_url};
use crate::utils::report::Report;
use crate::utils::DEFAULT_FEE_ABS;

struct TestRuntime {
    rt: RgbDirRuntime,
    signer: Option<TestnetSigner>,
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
    /*
    pub fn send(
        &mut self,
        recv_wlt: &mut TestRuntime,
        transfer_type: TransferType,
        contract_id: ContractId,
        amount: u64,
        sats: u64,
        report: Option<&Report>,
    ) -> (PathBuf, Tx) {
        let invoice = recv_wlt.invoice(contract_id, amount, transfer_type.into());
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
        self.mine_tx(&tx.txid(), false);
        recv_wlt.accept_transfer(consignment.clone(), report);
        self.sync();
        (consignment, tx)
    }
     */

    pub fn transfer(
        &mut self,
        invoice: RgbInvoice<ContractId>,
        sats: Option<u64>,
        fee: Option<u64>,
        broadcast: bool,
        report: Option<&Report>,
    ) -> (PathBuf, Tx) {
        static COUNTER: OnceLock<AtomicU32> = OnceLock::new();

        let mut counter = COUNTER.get_or_init(|| AtomicU32::new(0));
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
            .join("test-data")
            .with_file_name(format!("consignment-{consignment_no}"))
            .with_extension("rgb");
        self.mound
            .consign_to_file(invoice.scope, [terminal], &consignment)
            .unwrap();

        (consignment, tx)
    }

    pub fn sync(&mut self) {
        let indexer = self.get_indexer();
        self.wallet.update(&indexer).into_result().unwrap();
    }

    pub fn network(&self) -> Network { self.wallet.network() }

    fn get_indexer(&self) -> AnyIndexer { get_indexer(&self.indexer_url()) }

    pub fn indexer_url(&self) -> String { indexer_url(self.instance, self.network()) }

    pub fn sign_finalize(&self, psbt: &mut Psbt) {
        let _sig_count = psbt.sign(self.signer.as_ref().unwrap()).unwrap();
        psbt.finalize(self.wallet.descriptor());
    }

    pub fn sign_finalize_extract(&self, psbt: &mut Psbt) -> Tx {
        self.sign_finalize(psbt);
        psbt.extract().unwrap()
    }

    pub fn broadcast_tx(&self, tx: &Tx) { broadcast_tx(tx, &self.indexer_url()); }
}
