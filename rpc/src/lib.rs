#![warn(missing_docs)]

//! ISMP RPC Implementation.

use jsonrpsee::{core::RpcResult as Result, proc_macros::rpc};

use ismp_rust::consensus_client::ConsensusClientId;
use ismp_rust::host::ChainID;
use ismp_rust::router::{Request, Response};
use sc_client_api::{BlockBackend, ProofProvider};
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use std::collections::HashMap;
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

impl<Hash: std::fmt::Debug> Display for BlockNumberOrHash<Hash> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockNumberOrHash::Hash(hash) => write!(f, "{:?}", hash),
            BlockNumberOrHash::Number(block_num) => write!(f, "{}", block_num),
        }
    }
}

/// Contains a scale encoded Mmr Proof or Trie proof
#[derive(Serialize, Deserialize)]
pub struct Proof {
    /// Scale encoded Mmr proof or non-membership trie proof
    pub proof: Vec<u8>,
    /// Height at which proof was recovered
    pub height: u64,
}

#[derive(Serialize, Deserialize)]
pub struct LeafIndexQuery {
    pub source_chain: String,
    pub dest_chain: String,
    pub nonce: u64,
}

/// ISMP RPC methods.
#[rpc(client, server)]
pub trait ISMPApi<Hash>
where
    Hash: PartialEq + Eq + std::hash::Hash,
{
    /// Query full request data from the ismp pallet
    #[method(name = "ismp_queryRequests")]
    fn query_requests(&self, leaves: Vec<LeafIndexQuery>) -> Result<Vec<Request>>;

    /// Query full response data from the ismp pallet
    #[method(name = "ismp_queryResponses")]
    fn query_responses(&self, leaves: Vec<LeafIndexQuery>) -> Result<Vec<Response>>;

    /// Query mmr proof for some leaves
    #[method(name = "ismp_queryMmrProof")]
    fn query_mmr_proof(&self, leaves: Vec<LeafIndexQuery>) -> Result<Proof>;

    /// Query membership or non-membership proof for some keys
    #[method(name = "ismp_queryStateProof")]
    fn query_state_proof(&self, keys: Vec<Vec<u8>>) -> Result<Proof>;

    /// Query scale encoded consensus state
    #[method(name = "ismp_queryConsensusState")]
    fn query_consensus_state(&self, client_id: ConsensusClientId) -> Result<Vec<u8>>;

    /// Query ISMP Events that were deposited in a series of blocks
    /// Using String keys because HashMap fails to deserialize when key is not a String
    #[method(name = "ibc_queryEvents")]
    fn query_events(
        &self,
        block_numbers: Vec<BlockNumberOrHash<Hash>>,
    ) -> Result<HashMap<String, Vec<pallet_ismp::events::Event>>>;
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

impl<C, Block> ISMPApiServer<Block::Hash> for ISMPRpcHandler<C, Block>
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
    fn query_requests(&self, leaves: Vec<LeafIndexQuery>) -> Result<Vec<Request>> {
        todo!()
    }

    fn query_responses(&self, leaves: Vec<LeafIndexQuery>) -> Result<Vec<Response>> {
        todo!()
    }

    fn query_mmr_proof(&self, leaves: Vec<LeafIndexQuery>) -> Result<Proof> {
        todo!()
    }

    fn query_state_proof(&self, keys: Vec<Vec<u8>>) -> Result<Proof> {
        todo!()
    }

    fn query_consensus_state(&self, client_id: ConsensusClientId) -> Result<Vec<u8>> {
        todo!()
    }

    fn query_events(
        &self,
        _block_numbers: Vec<BlockNumberOrHash<Block::Hash>>,
    ) -> Result<HashMap<String, Vec<pallet_ismp::events::Event>>> {
        todo!()
    }
}
