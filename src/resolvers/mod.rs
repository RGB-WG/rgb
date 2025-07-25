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
#[cfg(any(feature = "resolver-esplora", feature = "resolver-esplora-async",))]
mod esplora;
// #[cfg(any(feature = "resolver-bitcoinrpc", feature = "resolver-bitcoinrpc-async"))]
// mod bitcoinrpc;

use core::iter;
#[cfg(feature = "std")]
use std::process::exit;

use amplify::IoError;
use bpstd::psbt::Utxo;
use bpstd::{ScriptPubkey, Terminal, Tx, Txid, UnsignedTx};
use rgb::WitnessStatus;

//#[cfg(feature = "resolver-electrum-async")]
//pub use self::electrum::ElectrumAsyncResolver;
#[cfg(feature = "resolver-electrum")]
pub use self::electrum::ElectrumResolver;
#[cfg(feature = "resolver-esplora-async")]
pub use self::esplora::EsploraAsyncResolver;
#[cfg(feature = "resolver-esplora")]
pub use self::esplora::EsploraResolver;

#[cfg(not(feature = "async"))]
pub trait Resolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError>;
    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError>;

    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>>;

    fn last_block_height(&self) -> Result<u64, ResolverError>;

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError>;
}

#[cfg(feature = "async")]
pub trait Resolver {
    async fn resolve_tx_async(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError>;
    async fn resolve_tx_status_async(&self, txid: Txid) -> Result<WitnessStatus, ResolverError>;

    async fn resolve_utxos_async(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>>;

    async fn last_block_height_async(&self) -> Result<u64, ResolverError>;

    async fn broadcast_async(&self, tx: &Tx) -> Result<(), ResolverError>;
}

#[derive(Default)]
pub struct MultiResolver {
    #[cfg(feature = "resolver-electrum")]
    electrum: Option<ElectrumResolver>,
    #[cfg(feature = "resolver-esplora")]
    esplora: Option<EsploraResolver>,
    // TODO: Implement Bitcoin RPC resolver
    // #[cfg(feature = "resolver-bitcoinrpc")]
    // bitcoinrpc: Option<NoResolver>,

    //#[cfg(feature = "resolver-electrum-async")]
    //electrum: Option<ElectrumAsyncResolver>,
    #[cfg(feature = "resolver-esplora-async")]
    esplora: Option<EsploraAsyncResolver>,
    // #[cfg(feature = "resolver-bitcoinrpc-async")]
    // bitcoinrpc: Option<NoResolver>,
}

#[derive(Copy, Clone)]
pub struct NoResolver;

impl NoResolver {
    fn call(&self) -> ! {
        #[cfg(feature = "std")]
        {
            eprintln!(
                "Error: no blockchain indexer specified; use either --esplora or --electrum \
                 argument"
            );
            exit(1);
        }
        #[cfg(not(feature = "std"))]
        panic!(
            "Error: no blockchain indexer, you need to use one of resolver-* features during the \
             compilation"
        );
    }
}

#[cfg(not(feature = "async"))]
impl Resolver for NoResolver {
    fn resolve_tx(&self, _txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> { self.call() }
    fn resolve_tx_status(&self, _txid: Txid) -> Result<WitnessStatus, ResolverError> { self.call() }
    fn resolve_utxos(
        &self,
        _iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        self.call();
        #[allow(unreachable_code)]
        iter::empty()
    }
    fn last_block_height(&self) -> Result<u64, ResolverError> { self.call() }
    fn broadcast(&self, _tx: &Tx) -> Result<(), ResolverError> { self.call() }
}

#[cfg(feature = "async")]
impl Resolver for NoResolver {
    async fn resolve_tx_async(&self, _txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        self.call()
    }
    async fn resolve_tx_status_async(&self, _txid: Txid) -> Result<WitnessStatus, ResolverError> {
        self.call()
    }
    async fn resolve_utxos_async(
        &self,
        _iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        self.call();
        #[allow(unreachable_code)]
        iter::empty()
    }
    async fn last_block_height_async(&self) -> Result<u64, ResolverError> { self.call() }
    async fn broadcast_async(&self, _tx: &Tx) -> Result<(), ResolverError> { self.call() }
}

impl MultiResolver {
    #[cfg(feature = "resolver-electrum")]
    pub fn new_electrum(url: &str) -> Result<Self, ResolverError> {
        Ok(Self { electrum: Some(ElectrumResolver::new(url)?), ..default!() })
    }
    #[cfg(feature = "resolver-esplora")]
    pub fn new_esplora(url: &str) -> Result<Self, ResolverError> {
        Ok(Self { esplora: Some(EsploraResolver::new(url)?), ..default!() })
    }
    #[allow(clippy::needless_update)]
    #[cfg(feature = "resolver-esplora-async")]
    pub fn new_esplora(url: &str) -> Result<Self, ResolverError> {
        Ok(Self { esplora: Some(EsploraAsyncResolver::new(url)?), ..default!() })
    }
    // #[cfg(feature = "resolver-bitcoinrpc")]
    // pub fn new_bitcoinrpc(_url: &str) -> Result<Self, ResolverError> { todo!() }
    pub fn new_absent() -> Result<Self, ResolverError> { Ok(Self::default()) }
}

#[cfg(not(feature = "async"))]
impl Resolver for MultiResolver {
    fn resolve_tx(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx(txid);
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_tx(txid);
        }
        // #[cfg(feature = "resolver-bitcoinrpc")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.resolve_tx(txid);
        // }
        NoResolver.call()
    }

    fn resolve_tx_status(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx_status(txid);
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_tx_status(txid);
        }
        // #[cfg(feature = "resolver-bitcoinrpc")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.resolve_tx_status(txid);
        // }
        NoResolver.call()
    }

    #[allow(unreachable_code)]
    fn resolve_utxos(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        }
        // #[cfg(feature = "resolver-bitcoinrpc")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.resolve_utxos(iter).collect::<Vec<_>>().into_iter();
        // }
        NoResolver.call();
        vec![].into_iter()
    }

    fn last_block_height(&self) -> Result<u64, ResolverError> {
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.last_block_height();
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.last_block_height();
        }
        // #[cfg(feature = "resolver-bitcoinrpc")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.last_block_height();
        // }
        NoResolver.call()
    }

    fn broadcast(&self, tx: &Tx) -> Result<(), ResolverError> {
        #[cfg(feature = "resolver-esplora")]
        if let Some(resolver) = &self.esplora {
            return resolver.broadcast(tx);
        }
        #[cfg(feature = "resolver-electrum")]
        if let Some(resolver) = &self.electrum {
            return resolver.broadcast(tx);
        }
        // #[cfg(feature = "resolver-bitcoinrpc")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.broadcast(tx);
        // }
        NoResolver.call()
    }
}

#[cfg(feature = "async")]
impl Resolver for MultiResolver {
    async fn resolve_tx_async(&self, txid: Txid) -> Result<Option<UnsignedTx>, ResolverError> {
        #[cfg(feature = "resolver-esplora-async")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx_async(txid).await;
        }
        // #[cfg(feature = "resolver-electrum-async")]
        // if let Some(resolver) = &self.electrum {
        //     return resolver.resolve_tx_async(txid).await;
        // }
        // #[cfg(feature = "resolver-bitcoinrpc-async")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.resolve_tx_async(txid).await;
        // }
        NoResolver.call()
    }

    async fn resolve_tx_status_async(&self, txid: Txid) -> Result<WitnessStatus, ResolverError> {
        #[cfg(feature = "resolver-esplora-async")]
        if let Some(resolver) = &self.esplora {
            return resolver.resolve_tx_status_async(txid).await;
        }
        // #[cfg(feature = "resolver-electrum-async")]
        // if let Some(resolver) = &self.electrum {
        //     return resolver.resolve_tx_status_async(txid).await;
        // }
        // #[cfg(feature = "resolver-bitcoinrpc-async")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.resolve_tx_status_async(txid).await;
        // }
        NoResolver.call()
    }

    #[allow(unreachable_code)]
    async fn resolve_utxos_async(
        &self,
        iter: impl IntoIterator<Item = (Terminal, ScriptPubkey)>,
    ) -> impl Iterator<Item = Result<Utxo, ResolverError>> {
        #[cfg(feature = "resolver-esplora-async")]
        if let Some(resolver) = &self.esplora {
            return resolver
                .resolve_utxos_async(iter)
                .await
                .collect::<Vec<_>>()
                .into_iter();
        }
        // #[cfg(feature = "resolver-electrum-async")]
        // if let Some(resolver) = &self.electrum {
        //     return resolver
        //         .resolve_utxos_async(iter)
        //         .await
        //         .collect::<Vec<_>>()
        //         .into_iter();
        // }
        // #[cfg(feature = "resolver-bitcoinrpc-async")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver
        //         .resolve_utxos_async(iter)
        //         .await
        //         .collect::<Vec<_>>()
        //         .into_iter();
        // }
        NoResolver.call();
        vec![].into_iter()
    }

    async fn last_block_height_async(&self) -> Result<u64, ResolverError> {
        #[cfg(feature = "resolver-esplora-async")]
        if let Some(resolver) = &self.esplora {
            return resolver.last_block_height_async().await;
        }
        // #[cfg(feature = "resolver-electrum-async")]
        // if let Some(resolver) = &self.electrum {
        //     return resolver.last_block_height_async().await;
        // }
        // #[cfg(feature = "resolver-bitcoinrpc-async")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.last_block_height_async().await;
        // }
        NoResolver.call()
    }

    async fn broadcast_async(&self, tx: &Tx) -> Result<(), ResolverError> {
        #[cfg(feature = "resolver-esplora-async")]
        if let Some(resolver) = &self.esplora {
            return resolver.broadcast_async(tx).await;
        }
        // #[cfg(feature = "resolver-electrum-async")]
        // if let Some(resolver) = &self.electrum {
        //     return resolver.broadcast_async(tx).await;
        // }
        // #[cfg(feature = "resolver-bitcoinrpc-async")]
        // if let Some(resolver) = &self.bitcoinrpc {
        //     return resolver.broadcast_async(tx).await;
        // }
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
