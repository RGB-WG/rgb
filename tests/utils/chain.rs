use std::env::VarError;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Once, OnceLock, RwLock};
use std::time::Duration;

use amplify::{s, Display};
use bpstd::{Network, Sats, Tx, Txid};
use bpwallet::indexers::esplora::Client as EsploraClient;
use bpwallet::AnyIndexer;
use electrum::{Client as ElectrumClient, ElectrumApi};
use once_cell::sync::Lazy;
use time::OffsetDateTime;

pub const ELECTRUM_1_REGTEST_URL: &str = "127.0.0.1:50001";
pub const ELECTRUM_2_REGTEST_URL: &str = "127.0.0.1:50002";
pub const ELECTRUM_3_REGTEST_URL: &str = "127.0.0.1:50003";
pub const ELECTRUM_MAINNET_URL: &str = "ssl://electrum.iriswallet.com:50003";
pub const ESPLORA_1_REGTEST_URL: &str = "http://127.0.0.1:8094/regtest/api";
pub const ESPLORA_2_REGTEST_URL: &str = "http://127.0.0.1:8095/regtest/api";
pub const ESPLORA_3_REGTEST_URL: &str = "http://127.0.0.1:8096/regtest/api";
pub const ESPLORA_MAINNET_URL: &str = "https://blockstream.info/api";

static INIT: Once = Once::new();
pub static INDEXER: OnceLock<Indexer> = OnceLock::new();

pub const INSTANCE_1: u8 = 1;
pub const INSTANCE_2: u8 = 2;
pub const INSTANCE_3: u8 = 3;

#[derive(Clone, Default, PartialEq, Eq, Debug, Display)]
#[display(lowercase)]
pub enum Indexer {
    Electrum,
    #[default]
    Esplora,
}
static MINER: Lazy<RwLock<Miner>> = Lazy::new(|| RwLock::new(Miner { no_mine_count: 0 }));

#[derive(Clone, Debug)]
pub struct Miner {
    no_mine_count: u32,
}

fn _service_base_name() -> String {
    match INDEXER.get().unwrap() {
        Indexer::Electrum => "bitcoind",
        Indexer::Esplora => "esplora",
    }
    .to_string()
}

fn _bitcoin_cli_cmd(instance: u8, args: Vec<&str>) -> String {
    let compose_file = PathBuf::from("tests").join("docker-compose.yml");
    let mut bitcoin_cli =
        vec![s!("-f"), compose_file.to_string_lossy().to_string(), s!("exec"), s!("-T")];
    let service_name = format!("{}_{instance}", _service_base_name());
    match INDEXER.get().unwrap() {
        Indexer::Electrum => bitcoin_cli.extend(vec![
            "-u".to_string(),
            "blits".to_string(),
            service_name,
            "bitcoin-cli".to_string(),
            "-regtest".to_string(),
        ]),
        Indexer::Esplora => bitcoin_cli.extend(vec![service_name, "cli".to_string()]),
    };
    let output = Command::new("docker")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .arg("compose")
        .args(bitcoin_cli)
        .args(&args)
        .output()
        .unwrap_or_else(|_| panic!("failed to call bitcoind with args {args:?}"));
    if !output.status.success() {
        println!("{output:?}");
        panic!("failed to get succesful output with args {args:?}");
    }
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

impl Miner {
    fn mine(&self, instance: u8, blocks: u32) -> bool {
        if self.no_mine_count > 0 {
            return false;
        }
        self.force_mine(instance, blocks)
    }

    fn force_mine(&self, instance: u8, blocks: u32) -> bool {
        _bitcoin_cli_cmd(instance, vec!["-rpcwallet=miner", "-generate", &blocks.to_string()]);
        _wait_indexer_sync(instance);
        true
    }

    fn stop_mining(&mut self) { self.no_mine_count += 1; }

    fn resume_mining(&mut self) {
        if self.no_mine_count > 0 {
            self.no_mine_count -= 1;
        }
    }
}

pub fn mine(resume: bool) { mine_custom(resume, INSTANCE_1, 1); }

pub fn mine_custom(resume: bool, instance: u8, blocks: u32) {
    let t_0 = OffsetDateTime::now_utc();
    if resume {
        resume_mining();
    }
    loop {
        if (OffsetDateTime::now_utc() - t_0).as_seconds_f32() > 120.0 {
            println!("forcibly breaking mining wait");
            resume_mining();
        }
        let mined = MINER.read().as_ref().unwrap().mine(instance, blocks);
        if mined {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

pub fn mine_but_no_resume() { mine_but_no_resume_custom(INSTANCE_1, 1); }

pub fn mine_but_no_resume_custom(instance: u8, blocks: u32) {
    let t_0 = OffsetDateTime::now_utc();
    loop {
        if (OffsetDateTime::now_utc() - t_0).as_seconds_f32() > 120.0 {
            println!("forcibly breaking mining wait");
            resume_mining();
        }
        let miner = MINER.write().unwrap();
        if miner.no_mine_count <= 1 {
            miner.force_mine(instance, blocks);
            break;
        }
        drop(miner);
        std::thread::sleep(Duration::from_millis(500));
    }
}

pub fn stop_mining() { MINER.write().unwrap().stop_mining() }

pub fn stop_mining_when_alone() {
    let t_0 = OffsetDateTime::now_utc();
    loop {
        if (OffsetDateTime::now_utc() - t_0).as_seconds_f32() > 120.0 {
            println!("forcibly breaking stop wait");
            stop_mining();
        }
        let mut miner = MINER.write().unwrap();
        if miner.no_mine_count == 0 {
            miner.stop_mining();
            break;
        }
        drop(miner);
        std::thread::sleep(Duration::from_millis(500));
    }
}

pub fn resume_mining() { MINER.write().unwrap().resume_mining() }

fn _get_connection_tuple() -> Vec<(u8, String)> {
    let serive_base_name = _service_base_name();
    vec![
        (INSTANCE_3, format!("{serive_base_name}_{INSTANCE_2}:18444")),
        (INSTANCE_2, format!("{serive_base_name}_{INSTANCE_3}:18444")),
    ]
}

pub fn connect_reorg_nodes() {
    for (instance, node_addr) in _get_connection_tuple() {
        _bitcoin_cli_cmd(instance, vec!["addnode", &node_addr, "onetry"]);
    }
    let t_0 = OffsetDateTime::now_utc();
    loop {
        if (OffsetDateTime::now_utc() - t_0).as_seconds_f32() > 20.0 {
            panic!("nodes are not syncing with each other")
        }
        let height_2 = get_height_custom(INSTANCE_2);
        let height_3 = get_height_custom(INSTANCE_3);
        if height_2 == height_3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

pub fn disconnect_reorg_nodes() {
    for (instance, node_addr) in _get_connection_tuple() {
        _bitcoin_cli_cmd(instance, vec!["disconnectnode", &node_addr]);
    }
}

pub fn get_height() -> u32 { get_height_custom(INSTANCE_1) }

pub fn get_height_custom(instance: u8) -> u32 {
    _bitcoin_cli_cmd(instance, vec!["getblockcount"])
        .parse::<u32>()
        .expect("could not parse blockcount")
}

pub fn initialize() {
    INIT.call_once(|| {
        INDEXER.get_or_init(|| match std::env::var("INDEXER") {
            Ok(val) if val.to_lowercase() == Indexer::Esplora.to_string() => Indexer::Esplora,
            Ok(val) if val.to_lowercase() == Indexer::Electrum.to_string() => Indexer::Electrum,
            Err(VarError::NotPresent) => Indexer::Esplora,
            _ => {
                panic!("invalid indexer. possible values: `esplora` (default), `electrum`")
            }
        });
        if std::env::var("SKIP_INIT").is_ok() {
            println!("skipping services initialization");
            return;
        }
        let start_services_file = PathBuf::from("tests").join("start_services.sh");
        println!("starting test services...");
        let output = Command::new(start_services_file)
            .env("PROFILE", INDEXER.get().unwrap().to_string())
            .output()
            .expect("failed to start test services");
        if !output.status.success() {
            println!("{output:?}");
            panic!("failed to start test services");
        }
        (INSTANCE_1..=INSTANCE_3).for_each(_wait_indexer_sync);
    });
}

pub fn indexer_url(instance: u8, network: Network) -> String {
    match (INDEXER.get().unwrap(), network, instance) {
        (Indexer::Electrum, Network::Mainnet, _) => ELECTRUM_MAINNET_URL,
        (Indexer::Electrum, Network::Regtest, INSTANCE_1) => ELECTRUM_1_REGTEST_URL,
        (Indexer::Electrum, Network::Regtest, INSTANCE_2) => ELECTRUM_2_REGTEST_URL,
        (Indexer::Electrum, Network::Regtest, INSTANCE_3) => ELECTRUM_3_REGTEST_URL,
        (Indexer::Esplora, Network::Mainnet, _) => ESPLORA_MAINNET_URL,
        (Indexer::Esplora, Network::Regtest, INSTANCE_1) => ESPLORA_1_REGTEST_URL,
        (Indexer::Esplora, Network::Regtest, INSTANCE_2) => ESPLORA_2_REGTEST_URL,
        (Indexer::Esplora, Network::Regtest, INSTANCE_3) => ESPLORA_3_REGTEST_URL,
        _ => unreachable!(),
    }
    .to_string()
}

pub fn get_indexer(indexer_url: &str) -> AnyIndexer {
    match INDEXER.get().unwrap() {
        Indexer::Electrum => {
            AnyIndexer::Electrum(Box::new(ElectrumClient::new(indexer_url).unwrap()))
        }
        Indexer::Esplora => {
            AnyIndexer::Esplora(Box::new(EsploraClient::new_esplora(indexer_url).unwrap()))
        }
    }
}

pub fn is_tx_mined(txid: Txid, indexer: &AnyIndexer) -> bool {
    match indexer {
        AnyIndexer::Electrum(indexer) => {
            use electrum::Param;
            let Ok(status) = indexer.raw_call("blockchain.transaction.get", vec![
                Param::String(txid.to_string()),
                Param::Bool(true),
            ]) else {
                return false;
            };
            status
                .get("confirmantions")
                .and_then(|confs| confs.as_u64())
                .map(|confs| confs > 0)
                .unwrap_or_default()
        }
        AnyIndexer::Esplora(indexer) | AnyIndexer::Mempool(indexer) => {
            let Ok(status) = indexer.tx_status(&txid) else {
                return false;
            };
            status.confirmed
        }
        _ => unreachable!(),
    }
}

pub fn broadcast_tx(tx: &Tx, indexer_url: &str) {
    match get_indexer(indexer_url) {
        AnyIndexer::Electrum(inner) => {
            inner.transaction_broadcast(tx).unwrap();
        }
        AnyIndexer::Esplora(inner) => {
            inner.broadcast(tx).unwrap();
        }
        _ => unreachable!("unsupported indexer"),
    }
}

pub fn broadcast_tx_and_mine(tx: &Tx, instance: u8) {
    broadcast_tx(tx, &indexer_url(instance, Network::Regtest));
    mine_custom(false, instance, 1);
}
fn _wait_indexer_sync(instance: u8) {
    let t_0 = OffsetDateTime::now_utc();
    let blockcount = get_height_custom(instance);
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let url = &indexer_url(instance, Network::Regtest);
        match INDEXER.get().unwrap() {
            Indexer::Electrum => {
                let electrum_client = ElectrumClient::new(url).unwrap();
                if electrum_client.block_header(blockcount as usize).is_ok() {
                    break;
                }
            }
            Indexer::Esplora => {
                let esplora_client = EsploraClient::new_esplora(url).unwrap();
                if esplora_client.block_hash(blockcount).is_ok() {
                    break;
                }
            }
        }
        if (OffsetDateTime::now_utc() - t_0).as_seconds_f32() > 25.0 {
            panic!("indexer not syncing with bitcoind");
        }
    }
}

fn _send_to_address(address: &str, sats: Option<u64>, instance: u8) -> String {
    let sats = Sats::from_sats(sats.unwrap_or(100_000_000));
    let btc = format!("{}.{:0>8}", sats.btc_floor(), sats.sats_rem());
    _bitcoin_cli_cmd(instance, vec!["-rpcwallet=miner", "sendtoaddress", address, &btc])
}

pub fn fund_wallet(address: String, sats: Option<u64>, instance: u8) -> String {
    let txid = _send_to_address(&address, sats, instance);
    mine_custom(false, instance, 1);
    txid
}
