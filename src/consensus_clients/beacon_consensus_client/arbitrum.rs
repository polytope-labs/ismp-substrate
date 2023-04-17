use crate::consensus_clients::{
    beacon_consensus_client::{
        presets::ARB_ROLLUP_CONTRACT,
        state_machine_ids::ARBITRUM_ID,
        utils::{get_contract_storage_root, get_value_from_proof, to_bytes_32},
    },
    consensus_client_ids::ETHEREUM_CONSENSUS_CLIENT_ID,
};
use ethabi::{
    ethereum_types::{H160, H256, H64, U256},
    Token,
};
use ismp_rs::{
    consensus_client::{IntermediateState, StateCommitment, StateMachineHeight, StateMachineId},
    error::Error,
};
use rlp_derive::RlpEncodable;
use alloc::string::ToString;

/// https://github.com/OffchainLabs/go-ethereum/blob/8c5b9339ca9043d2b8fb5e35814a64e7e9ff7c9b/core/types/block.go#L70
#[derive(RlpEncodable, codec::Encode, codec::Decode)]
pub struct Header {
    pub parent_hash: H256,
    pub uncle_hash: H256,
    pub coinbase: H160,
    pub state_root: H256,
    pub transactions_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Vec<u8>,
    pub difficulty: U256,
    pub number: U256,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    // This is the sendRoot, a 32 byte hash
    // https://github.com/OffchainLabs/go-ethereum/blob/8c5b9339ca9043d2b8fb5e35814a64e7e9ff7c9b/core/types/arb_types.go#L457
    pub extra_data: Vec<u8>,
    pub mix_hash: H256,
    pub nonce: H64,
    pub base_fee_per_gas: U256,
}

impl Header {
    pub fn hash(self) -> [u8; 32] {
        let encoding = rlp::encode(&self);
        sp_io::hashing::keccak_256(&encoding.to_vec())
    }
}

#[derive(codec::Encode, codec::Decode)]
pub struct GlobalState {
    pub block_hash: [u8; 32],
    pub send_root: [u8; 32],
    pub inbox_position: u64,
    pub position_in_message: u64,
}

impl GlobalState {
    /// https://github.com/OffchainLabs/nitro/blob/5e9f4228e6418b114a5aea0aa7f2f0cc161b67c0/contracts/src/state/GlobalState.sol#L16
    pub fn hash(&self) -> [u8; 32] {
        // abi encode packed
        let mut buf = Vec::new();
        buf.extend_from_slice("Global state:".as_bytes());
        buf.extend_from_slice(&self.block_hash[..]);
        buf.extend_from_slice(&self.send_root[..]);
        buf.extend_from_slice(&self.inbox_position.to_be_bytes()[..]);
        buf.extend_from_slice(&self.position_in_message.to_be_bytes()[..]);
        sp_io::hashing::keccak_256(&buf)
    }
}

#[derive(codec::Encode, codec::Decode)]
pub enum MachineStatus {
    Running = 0,
    Finished = 1,
    Errored = 2,
    TooFar = 3,
}

#[derive(codec::Encode, codec::Decode)]
pub struct ArbitrumPayloadProof {
    /// Arbitrum header that corresponds to the node being created
    pub arbitrum_header: Header,
    /// Global State as recorded in the NodeCreated event that was emitted for this node
    pub global_state: GlobalState,
    /// Machine status as recorded in the NodeCreated event that was emitted for this node
    pub machine_status: MachineStatus,
    /// Inbox max count as recorded in the NodeCreated event that was emitted for this node
    pub inbox_max_count: U256,
    /// Key used to store the node  in the _nodes mapping in the RollupCore as recorded in the
    /// latestNodeCreated field of the NodeCreated event
    pub node_number: u64,
    /// Proof for the state_hash field in the Node struct inside the _nodes mapping in the
    /// RollupCore
    pub storage_proof: Vec<Vec<u8>>,
    /// RollupCore contract proof in the ethereum world trie
    pub contract_proof: Vec<Vec<u8>>,
}

/// Storage layout slot for the nodes map in the Rollup Contract
pub(super) const NODES_SLOT: u8 = 3;

/// https://github.com/OffchainLabs/nitro/blob/5e9f4228e6418b114a5aea0aa7f2f0cc161b67c0/contracts/src/rollup/RollupLib.sol#L59
fn get_state_hash(
    global_state: GlobalState,
    machine_status: MachineStatus,
    inbox_max_count: U256,
) -> [u8; 32] {
    // abi encode packed
    let mut buf = Vec::new();
    buf.extend_from_slice(&global_state.hash()[..]);
    let mut inbox = Vec::with_capacity(32);
    inbox_max_count.to_big_endian(&mut inbox);
    buf.extend_from_slice(&inbox);
    buf.extend_from_slice((machine_status as u8).to_be_bytes().as_slice());
    sp_io::hashing::keccak_256(&buf)
}

/// nodes are stored in a mapping with keys as u64, since the state_hash is the first value in the
/// Node struct we don't need any offset
fn derive_key(key: u64, slot: u8) -> Vec<u8> {
    ethabi::encode(&[Token::Uint(U256::from(key)), Token::Int(U256::from(slot))])
}

pub(super) fn verify_arbitrum_payload(
    payload: ArbitrumPayloadProof,
    root: &[u8],
) -> Result<IntermediateState, Error> {
    let root = to_bytes_32(root)?;
    let root = H256::from_slice(&root[..]);

    let storage_root =
        get_contract_storage_root(payload.contract_proof, &ARB_ROLLUP_CONTRACT, root)?;

    if &payload.global_state.send_root[..] != &payload.arbitrum_header.extra_data {
        Err(Error::ImplementationSpecific(
            "Arbitrum header extra data does not match send root in global state".to_string(),
        ))?
    }

    let block_number = payload.arbitrum_header.number.low_u64();
    let timestamp = payload.arbitrum_header.timestamp;
    let state_root = payload.arbitrum_header.state_root.0;

    let header_hash = payload.arbitrum_header.hash();
    if payload.global_state.block_hash != header_hash {
        Err(Error::ImplementationSpecific(
            "Arbitrum header hash does not match block hash in global state".to_string(),
        ))?
    }

    let state_hash =
        get_state_hash(payload.global_state, payload.machine_status, payload.inbox_max_count);

    let state_hash_key = derive_key(payload.node_number, NODES_SLOT);
    let proof_value = get_value_from_proof(state_hash_key, storage_root, payload.storage_proof)?
        .ok_or_else(|| {
            Error::MembershipProofVerificationFailed("Value not found in proof".to_string())
        })?;

    if &proof_value[..] != &state_hash[..] {
        Err(Error::MembershipProofVerificationFailed(
            "State hash from proof does not match calculated state hash".to_string(),
        ))?
    }

    Ok(IntermediateState {
        height: StateMachineHeight {
            id: StateMachineId {
                state_id: ARBITRUM_ID,
                consensus_client: ETHEREUM_CONSENSUS_CLIENT_ID,
            },
            height: block_number,
        },
        commitment: StateCommitment { timestamp, ismp_root: [0u8; 32], state_root },
    })
}
