// Wallet Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
// Copyright (C) 2025 RGB Consortium, Switzerland.
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

#[cfg(feature = "resolver-electrum")]
mod electrum;
#[cfg(any(feature = "resolver-esplora", feature = "resolver-mempool"))]
mod esplora;
#[cfg(feature = "resolver-bitcoinrpc")]
mod bitcoinrpc;

use core::iter;
#[cfg(feature = "std")]
use std::process::exit;

use amplify::IoError;
use bpstd::psbt::Utxo;
use bpstd::{ScriptPubkey, Terminal, Tx, Txid, UnsignedTx};
#[cfg(feature = "resolver-electrum")]
pub use electrum::ElectrumResolver;
#[cfg(feature = "resolver-esplora")]
pub use esplora::EsploraResolver;
#[cfg(feature = "resolver-mempool")]
pub use esplora::EsploraResolver as MempoolResolver;
use rgb::WitnessStatus;

pub trait Resolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError>;
    #[cfg(feature = "async")]
    async fn resolve_tx_async(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError>;
    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError>;
    #[cfg(feature = "async")]
    async fn resolve_tx_status_async(&self, txid: Txid) -> Result<WitnessStatus, ResolverError>;

    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>>;
    #[cfg(feature = "async")]
    async fn resolve_utxos_async(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>>;

    fn last_block_height(&self) -> Result<u64, ResolverError>;
    #[cfg(feature = "async")]
    async fn last_block_height_async(&self) -> Result<u64, ResolverError>;

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError>;
    #[cfg(feature = "async")]
    async fn broadcast_async(&self, tx: &Tx) -> Result<(), ResolverError>;
}

#[derive(Default)]
pub struct MultiResolver {
    #[cfg(feature = "resolver-electrum")]
    electrum: Option<ElectrumResolver>,
    #[cfg(feature = "resolver-esplora")]
    esplora: Option<EsploraResolver>,
    #[cfg(feature = "resolver-mempool")]
    mempool: Option<MempoolResolver>,
    #[cfg(feature = "resolver-bitcoinrpc")]
    // TODO: Implement Bitcoin RPC resolver
    bitcoinrpc: Option<NoResolver>,
}

#[derive(Copy, Clone)]
pub struct NoResolver;

impl NoResolver {
    fn call(&self) -> ! {
        eprintln!(
            "Error: no blockchain indexer specified; use either --esplora --mempool or --electrum \
             argument"
        );
        #[cfg(feature = "std")]
        exit(1);
    }
}

impl Resolver for NoResolver {
    fn resolve_tx(&self, _txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> { self.call() }
    #[cfg(feature = "async")]
    async fn resolve_tx_async(&self, _txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        self.call()
    }

    fn resolve_tx_status(&self, _txid: Txid) -> Result<WitnessStatus, ResolverError> { self.call() }
    #[cfg(feature = "async")]
    async fn resolve_tx_status_async(&self, _txid: Txid) -> Result<WitnessStatus, ResolverError> {
        self.call()
    }

    fn resolve_utxos(
        &self,
        _iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        self.call();
        #[allow(unreachable_code)]
        iter::empty()
    }
    #[cfg(feature = "async")]
    async fn resolve_utxos_async(
        &self,
        _iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        self.call();
        #[allow(unreachable_code)]
        iter::empty()
    }

    fn last_block_height(&self) -> Result<u64, ResolverError> { self.call() }
    #[cfg(feature = "async")]
    async fn last_block_height_async(&self) -> Result<u64, ResolverError> { self.call() }

    fn broadcast(&self, _tx: &Tx) -> Result<(), ResolverError> { self.call() }
    #[cfg(feature = "async")]
    async fn broadcast_async(&self, _tx: &Tx) -> Result<(), ResolverError> { self.call() }
}

impl MultiResolver {
    #[cfg(feature = "resolver-electrum")]
    pub fn new_electrum(_url: &str) -> Self { todo!() }
    #[cfg(feature = "resolver-esplora")]
    pub fn new_esplora(_url: &str) -> Self { todo!() }
    #[cfg(feature = "resolver-mempool")]
    pub fn new_mempool(_url: &str) -> Self { todo!() }
    #[cfg(feature = "resolver-bitcoinrpc")]
    pub fn new_bitcoinrpc(_url: &str) -> Self { todo!() }
    pub fn new_absent() -> Self { Self::default() }
}

impl Resolver for MultiResolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.resolve_tx(txid);
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx(txid);
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_tx(txid);
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.resolve_tx(txid);
        }
        NoResolver.call()
    }
    #[cfg(feature = "async")]
    async fn resolve_tx_async(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.resolve_tx_async(txid).await;
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx_async(txid).await;
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_tx_async(txid).await;
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.resolve_tx_async(txid).await;
        }
        NoResolver.call()
    }

    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.resolve_tx_status(txid);
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx_status(txid);
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_tx_status(txid);
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.resolve_tx_status(txid);
        }
        NoResolver.call()
    }
    #[cfg(feature = "async")]
    async fn resolve_tx_status_async(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.resolve_tx_status_async(txid).await;
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx_status_async(txid).await;
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_tx_status_async(txid).await;
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.resolve_tx_status_async(txid).await;
        }
        NoResolver.call()
    }

    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        }
        NoResolver.call()
    }
    #[cfg(feature = "async")]
    async fn resolve_utxos_async(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver
                .resolve_utxos_async(iter)
                .await
                .collect::<Vec<_>>()
                .into_iter();
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver
                .resolve_utxos_async(iter)
                .await
                .collect::<Vec<_>>()
                .into_iter();
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver
                .resolve_utxos_async(iter)
                .await
                .collect::<Vec<_>>()
                .into_iter();
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver
                .resolve_utxos_async(iter)
                .await
                .collect::<Vec<_>>()
                .into_iter();
        }
        NoResolver.call()
    }

    fn last_block_height(&self) -> Result<u64, ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.last_block_height();
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.last_block_height();
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.last_block_height();
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.last_block_height();
        }
        NoResolver.call()
    }
    #[cfg(feature = "async")]
    async fn last_block_height_async(&self) -> Result<u64, ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.last_block_height_async().await;
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.last_block_height_async().await;
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.last_block_height_async().await;
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.last_block_height_async().await;
        }
        NoResolver.call()
    }

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.broadcast(tx);
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.broadcast(tx);
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.broadcast(tx);
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.broadcast(tx);
        }
        NoResolver.call()
    }
    #[cfg(feature = "async")]
    async fn broadcast_async(&self, tx: &Tx) -> Result<(), ResolverError> {
        #[cfg(feature = "resolver-mempool")]
        if let Some(resolver) = &self.mempool {
            return resolver.broadcast_async(tx).await;
        }
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.broadcast_async(tx).await;
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.broadcast_async(tx).await;
        }
        #[cfg(feature = "resolver-bitcoinrpc")]
        if let Some(resolver) = &self.bitcoinrpc {
            return resolver.broadcast_async(tx).await;
        }
        NoResolver.call()
    }
}

#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum ResolverError {
    Io(IoError),

    /// cannot connect to the indexer server.
    Connectivity,

    /// internal resolver error on the client side.
    Local,

    /// indexer uses invalid protocol.
    Protocol,

    /// invalid caller business logic.
    Logic,

    /// the indexer server has returned an error "{0}"
    ServerSide(String),
}
