use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;

use amplify::RawArray;
use bitcoin::hashes::Hash;
use bitcoin::ScriptBuf;
use bp::{Tx, TxIn, TxOut, VarIntArray};
use electrum_client::{ElectrumApi, Error, ListUnspentRes};
use rgbstd::resolvers::ResolveHeight;
use rgbstd::validation::{ResolveTx, TxResolverError};

use super::*;
use crate::descriptor::DeriveInfo;
use crate::wallet::{MiningStatus, Utxo};

#[derive(Wrapper, WrapperMut, From)]
#[wrapper(Deref)]
#[wrapper_mut(DerefMut)]
pub struct BlockchainResolver(electrum_client::Client);

impl BlockchainResolver {
    pub fn with(url: &str) -> Result<Self, electrum_client::Error> {
        electrum_client::Client::new(url).map(Self)
    }
}

impl Resolver for BlockchainResolver {
    fn resolve_utxo<'s>(
        &mut self,
        scripts: BTreeMap<DeriveInfo, ScriptBuf>,
    ) -> Result<BTreeSet<Utxo>, String> {
        Ok(self
            .batch_script_list_unspent(scripts.values().map(ScriptBuf::as_script))
            .map_err(|err| err.to_string())?
            .into_iter()
            .zip(scripts.into_keys())
            .flat_map(|(list, derivation)| {
                list.into_iter()
                    .map(move |res| Utxo::with(derivation.clone(), res))
            })
            .collect())
    }
}

impl ResolveTx for BlockchainResolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Tx, TxResolverError> {
        let tx = self
            .0
            .transaction_get(&bitcoin::Txid::from_byte_array(txid.to_raw_array()))
            .map_err(|err| match err {
                Error::Message(_) | Error::Protocol(_) => TxResolverError::Unknown(txid),
                err => TxResolverError::Other(txid, err.to_string()),
            })?;
        Ok(Tx {
            version: (tx.version as u8)
                .try_into()
                .expect("non-consensus tx version"),
            inputs: VarIntArray::try_from_iter(tx.input.into_iter().map(|txin| TxIn {
                prev_output: Outpoint::new(
                    txin.previous_output.txid.to_byte_array().into(),
                    txin.previous_output.vout,
                ),
                sig_script: txin.script_sig.to_bytes().into(),
                sequence: txin.sequence.0.into(),
            }))
            .expect("consensus-invalid transaction"),
            outputs: VarIntArray::try_from_iter(tx.output.into_iter().map(|txout| TxOut {
                value: txout.value.into(),
                script_pubkey: txout.script_pubkey.to_bytes().into(),
            }))
            .expect("consensus-invalid transaction"),
            lock_time: tx.lock_time.to_consensus_u32().into(),
        })
    }
}

impl ResolveHeight for BlockchainResolver {
    type Error = Infallible;
    fn resolve_height(&mut self, _txid: Txid) -> Result<u32, Self::Error> {
        // TODO: find a way how to resolve transaction height
        Ok(0)
    }
}

impl Utxo {
    fn with(derivation: DeriveInfo, res: ListUnspentRes) -> Self {
        Utxo {
            status: if res.height == 0 {
                MiningStatus::Mempool
            } else {
                MiningStatus::Blockchain(res.height as u32)
            },
            outpoint: Outpoint::new(
                Txid::from_raw_array(res.tx_hash.to_byte_array()),
                res.tx_pos as u32,
            ),
            derivation,
            amount: res.value,
        }
    }
}
