#![warn(missing_docs)]

//! ISMP RPC Implementation.

use jsonrpsee::{core::RpcResult as Result, proc_macros::rpc};

use sc_client_api::{BlockBackend, ProofProvider};
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use std::{fmt::Display, sync::Arc};

/// A type that could be a block number or a block hash
#[derive(Clone, Hash, Debug, PartialEq, Eq, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockNumberOrHash<Hash> {
    /// Block hash
    Hash(Hash),
    /// Block number
    Number(u32),
}

/// A type that could be a block number or a block hash
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeightAndTimestamp {
    /// Height
    pub height: u64,
    /// Timestamp nano seconds
    pub timestamp: u64,
}

impl<Hash: std::fmt::Debug> Display for BlockNumberOrHash<Hash> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockNumberOrHash::Hash(hash) => write!(f, "{:?}", hash),
            BlockNumberOrHash::Number(block_num) => write!(f, "{}", block_num),
        }
    }
}

/// Proof for a set of keys
#[derive(Serialize, Deserialize)]
pub struct Proof {
    /// Mmr proof or non-membership trie proof
    pub proof: Vec<Vec<u8>>,
    /// Height at which proof was recovered
    pub height: u64,
}

/// ISMP RPC methods.
#[rpc(client, server)]
pub trait ISMPApi<BlockNumber, Hash>
where
    Hash: PartialEq + Eq + std::hash::Hash,
{
    /// Query requests from the ismp pallet
    #[method(name = "ismp_queryRequests")]
    fn query_requests(&self) -> Result<()>;
}

/// An implementation of ISMP specific RPC methods.
pub struct ISMPRpcHandler<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> ISMPRpcHandler<C, B> {
    /// Create new `ISMPRpcHandler` with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> ISMPApiServer<<<Block as BlockT>::Header as HeaderT>::Number, Block::Hash>
    for ISMPRpcHandler<C, Block>
where
    Block: BlockT,
    C: Send
        + Sync
        + 'static
        + ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + ProofProvider<Block>
        + BlockBackend<Block>,
{
    fn query_requests(&self) -> Result<()> {
        Ok(())
    }
}
