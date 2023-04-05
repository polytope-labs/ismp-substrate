use codec::{Decode, Encode};
use core::time::Duration;
use ethabi::Token;
use ismp_rust::{
    consensus_client::{
        ConsensusClient, ConsensusClientId, IntermediateState, StateCommitment, StateMachineHeight,
        StateMachineId, ETHEREUM_CONSENSUS_CLIENT_ID,
    },
    error::Error,
    host::ISMPHost,
    messaging::Proof,
};
use patricia_merkle_trie::{keccak::KeccakHasher, EIP1186Layout, StorageProof};
use primitive_types::{H256, U256};
use rlp::{Decodable, Rlp};
use rlp_derive::RlpDecodable;
use sync_committee_primitives::derived_types::{LightClientState, LightClientUpdate};
use tiny_keccak::{Hasher, Keccak};
use trie_db::{DBValue, Trie, TrieDBBuilder};

#[derive(Debug, Encode, Decode, Clone)]
pub struct ConsensusState {
    pub frozen_height: Option<u64>,
    pub light_client_state: LightClientState,
}

#[derive(Encode, Decode)]
pub struct Misbehaviour {
    pub update_1: LightClientUpdate,
    pub update_2: LightClientUpdate,
}

#[derive(Encode, Decode)]
pub enum BeaconMessage {
    ConsensusUpdate(LightClientUpdate),
    Misbehaviour(Misbehaviour),
}

// Slot for requests map
const REQ_SLOT: u8 = 1;
// Slot for responses map
const _RESP_SLOT: u8 = 2;
const CONTRACT_ADDRESS: &'static str = "0x0000";
#[derive(Encode, Decode, Clone)]
pub struct EvmStateProof {
    pub contract_account_proof: Vec<Vec<u8>>,
    pub actual_key_proof: Vec<Vec<u8>>,
}

/// The ethereum account stored in the global state trie.
#[derive(RlpDecodable, Debug)]
struct Account {
    _nonce: u64,
    _balance: U256,
    storage_root: H256,
    _code_hash: H256,
}

// TODO:  Unbonding period for ethereum
const UNBONDING_PERIOD: u64 = 14;
// number of seconds in a day
const DAY: u64 = 24 * 60 * 60;
const EXECUTION_PAYLOAD_STATE_ID: u64 = 1;

impl ConsensusClient for ConsensusState {
    fn verify(
        &self,
        host: &dyn ISMPHost,
        trusted_consensus_state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        let beacon_message = BeaconMessage::decode(&mut &proof[..]).map_err(|_| {
            Error::ImplementationSpecific(format!("Cannot decode beacon message {:?}", proof))
        })?;

        let light_client_update = match beacon_message {
            BeaconMessage::ConsensusUpdate(update) => update.clone(),
            _ => return Err(Error::CannotHandleConsensusMessage),
        };

        let light_client_state = LightClientState::decode(&mut &trusted_consensus_state[..])
            .map_err(|_| {
                Error::ImplementationSpecific(format!(
                    "Cannot decode trusted consensus state {:?}",
                    trusted_consensus_state
                ))
            })?;

        let height = light_client_update.finalized_header.slot;
        // Ensure consensus client is not frozen
        let is_frozen = if let Some(frozen_height) = self.frozen_height {
            light_client_update.finalized_header.slot >= frozen_height
        } else {
            false
        };

        if is_frozen {
            return Err(Error::FrozenConsensusClient { id: self.consensus_id() })
        }

        // check that the client hasn't elapsed unbonding period
        let timestamp = light_client_update.execution_payload.timestamp;
        if host.host_timestamp() - host.consensus_update_time(self.consensus_id())? >=
            self.unbonding_period()
        {
            return Err(Error::ImplementationSpecific(format!(
                "Unbonding period elapsed for host {:?} and consensus id {:?}",
                host.host(),
                self.consensus_id()
            )))
        }

        let no_codec_light_client_state = light_client_state.clone().try_into().map_err(|_| {
            Error::ImplementationSpecific(format!(
                "Cannot convert light client state {:?} to no codec type",
                light_client_state
            ))
        })?;
        let no_codec_light_client_update =
            light_client_update.clone().try_into().map_err(|_| {
                Error::ImplementationSpecific(format!(
                    "Cannot convert light client update {:?} to no codec type",
                    light_client_update
                ))
            })?;

        let new_light_client_state = sync_committee_verifier::verify_sync_committee_attestation(
            no_codec_light_client_state,
            no_codec_light_client_update,
        )
        .map_err(|_| Error::ConsensusProofVerificationFailed { id: self.consensus_id() })?;

        let mut intermediate_states = vec![];

        let commitment_root = light_client_update.execution_payload.state_root.clone();
        let intermediate_state = construct_intermediate_state(
            EXECUTION_PAYLOAD_STATE_ID,
            self.consensus_id(),
            height,
            timestamp,
            commitment_root,
        );

        intermediate_states.push(intermediate_state);

        let encoded_new_light_client_state: LightClientState =
            new_light_client_state.clone().try_into().map_err(|_| {
                Error::ImplementationSpecific(format!(
                    "Cannot convert light client state {:?} to codec type",
                    new_light_client_state
                ))
            })?;

        let new_proof = encoded_new_light_client_state.encode().to_vec();

        Ok((new_proof, intermediate_states))
    }

    fn consensus_id(&self) -> ConsensusClientId {
        ETHEREUM_CONSENSUS_CLIENT_ID
    }

    fn unbonding_period(&self) -> Duration {
        Duration::from_secs(UNBONDING_PERIOD * DAY)
    }

    fn verify_membership(
        &self,
        _host: &dyn ISMPHost,
        key: Vec<u8>,
        commitment: Vec<u8>,
        proof: &Proof,
    ) -> Result<(), Error> {
        let evm_state_proof = decode_evm_state_proof(proof)?;
        // the raw account data stored in the state proof:
        let contract_account =
            derive_contract_account_from_proof(evm_state_proof.clone(), commitment.clone())?;
        let result = generate_result_from_proof_db(key.clone(), commitment, evm_state_proof)?
            .ok_or_else(|| {
                Error::ImplementationSpecific(format!("There is no DB value from key {:?}", key,))
            })?;

        if result != contract_account.storage_root.0.to_vec() {
            return Err(Error::ImplementationSpecific(format!(
                "Could not verify membership {:?}",
                key
            )))
        }

        Ok(())
    }

    fn verify_non_membership(
        &self,
        _host: &dyn ISMPHost,
        key: Vec<u8>,
        commitment: Vec<u8>,
        proof: &Proof,
    ) -> Result<(), Error> {
        let evm_state_proof = decode_evm_state_proof(proof)?;

        let _contract_account =
            derive_contract_account_from_proof(evm_state_proof.clone(), commitment.clone())?;

        let result = generate_result_from_proof_db(key.clone(), commitment, evm_state_proof)?;

        if result.is_some() {
            return Err(Error::ImplementationSpecific(format!("membership is present in {:?}", key)))
        }

        Ok(())
    }

    fn is_frozen(&self, _host: &dyn ISMPHost, _id: ConsensusClientId) -> Result<bool, Error> {
        todo!()
    }
}

fn construct_intermediate_state(
    state_id: u64,
    consensus_client_id: u64,
    height: u64,
    timestamp: u64,
    commitment_root: Vec<u8>,
) -> IntermediateState {
    let state_machine_id = StateMachineId { state_id, consensus_client: consensus_client_id };

    let state_machine_height = StateMachineHeight { id: state_machine_id, height };

    let state_commitment = StateCommitment { timestamp, commitment_root };

    let intermediate_state =
        IntermediateState { height: state_machine_height, commitment: state_commitment };

    intermediate_state
}

fn decode_evm_state_proof(proof: &Proof) -> Result<EvmStateProof, Error> {
    let proof_vec = proof.proof.clone();
    let evm_state_proof = EvmStateProof::decode(&mut &proof_vec[..]).map_err(|_| {
        Error::ImplementationSpecific(format!("Cannot decode evm state proof {:?}", proof_vec))
    })?;

    Ok(evm_state_proof)
}

fn derive_contract_account_from_proof(
    evm_state_proof: EvmStateProof,
    commitment: Vec<u8>,
) -> Result<Account, Error> {
    let db =
        StorageProof::new(evm_state_proof.contract_account_proof).into_memory_db::<KeccakHasher>();
    let root = H256::from_slice(&commitment[..]);
    let trie = TrieDBBuilder::<EIP1186Layout<KeccakHasher>>::new(&db, &root).build();
    let contract_address = CONTRACT_ADDRESS.parse().map_err(|_| {
        Error::ImplementationSpecific(format!(
            "An error occurred when trying to parse contract address {:?}",
            CONTRACT_ADDRESS
        ))
    })?;
    let key = ethabi::encode(&[Token::String(contract_address)]);
    let result = trie
        .get(&key)
        .map_err(|_| {
            Error::ImplementationSpecific(format!(
                "An error occurred when trying to derive DB Value from key {:?}",
                key
            ))
        })?
        .ok_or_else(|| {
            Error::ImplementationSpecific(format!("There is no DB value from key {:?}", key,))
        })?;

    // the raw account data stored in the state proof:
    let contract_account = Account::decode(&mut Rlp::new(&result)).map_err(|_| {
        Error::ImplementationSpecific(format!(
            "Error decoding contract account from key {:?}",
            &result
        ))
    })?;

    Ok(contract_account)
}

// Generates slot index for the key based using Keccak 256
fn generate_slot_index(key: [u8; 32], slot: u8) -> [u8; 32] {
    let input = ethabi::encode(&[Token::Int(U256::from(slot)), Token::FixedBytes(key.to_vec())]);

    let mut hasher = Keccak::v256();
    hasher.update(&input);

    let mut result = [0u8; 32];
    hasher.finalize(&mut result);

    result
}

fn generate_result_from_proof_db(
    key: Vec<u8>,
    commitment: Vec<u8>,
    evm_state_proof: EvmStateProof,
) -> Result<Option<DBValue>, Error> {
    // generate slot index for key
    let maybe_key_bytes: Result<[u8; 32], _> = key.clone().try_into();

    let key_bytes: [u8; 32] = match maybe_key_bytes {
        Ok(array) => array,
        Err(_) => {
            return Err(Error::ImplementationSpecific(format!(
                "key must have exactly 32 elements {:?}",
                &key
            )))
        }
    };

    let slot_index_for_key = generate_slot_index(key_bytes, REQ_SLOT);

    // ProofDB using key proof
    let proof_db = StorageProof::new(evm_state_proof.actual_key_proof.clone())
        .into_memory_db::<KeccakHasher>();
    let root = H256::from_slice(&commitment[..]);
    let trie = TrieDBBuilder::<EIP1186Layout<KeccakHasher>>::new(&proof_db, &root).build();

    let result = trie.get(&slot_index_for_key).map_err(|_| {
        Error::ImplementationSpecific(format!(
            "An error occurred when trying to derive DB Value from key {:?}",
            key
        ))
    })?;

    Ok(result)
}
