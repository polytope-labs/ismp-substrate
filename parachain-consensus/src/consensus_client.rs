use core::{marker::PhantomData, time::Duration};

use codec::{Decode, Encode};
use hex_literal::hex;
use ismp::{
    consensus_client::{
        ConsensusClient, ConsensusClientId, IntermediateState, StateCommitment, StateMachineHeight,
        StateMachineId,
    },
    error::Error,
    host::ISMPHost,
    messaging::Proof,
    router::RequestResponse,
};
use merkle_mountain_range::MerkleProof;
use primitive_types::H256;
use sp_consensus_aura::AURA_ENGINE_ID;
use sp_runtime::{
    traits::{BlakeTwo256, Header, Keccak256},
    DigestItem,
};
use sp_trie::{LayoutV0, StorageProof, Trie, TrieDBBuilder};

use crate::RelayChainOracle;

struct ParachainConsensusClient<T>(PhantomData<T>);

/// Information necessary to prove the sibling parachain's finalization to this
/// parachain.
#[derive(Encode, Decode)]
pub struct ParachainConsensusProof {
    /// List of para ids contained in the proof
    pub para_ids: Vec<u32>,
    /// Height of the relay chain for the given proof
    pub relay_height: u32,
    /// Storage proof for the parachain headers
    pub storage_proof: Vec<Vec<u8>>,
}

/// Hashing algorithm for the state proof
#[derive(Encode, Decode)]
pub enum HashAlgorithm {
    Keccak,
    Blake2,
}

#[derive(Encode, Decode)]
pub struct ParachainStateProof {
    /// Algorithm to use for state proof verification
    pub hasher: HashAlgorithm,
    /// Storage proof for the parachain headers
    pub storage_proof: Vec<Vec<u8>>,
}

/// Static key for parachain headers in the relay chain storage
const PARACHAIN_HEADS_KEY: [u8; 32] =
    hex!("cd710b30bd2eab0352ddcc26417aa1941b3c252fcb29d88eff4f3de5de4476c3");

/// The `ConsensusEngineId` of ISMP.
pub const ISMP_ID: sp_runtime::ConsensusEngineId = *b"ISMP";

/// ConsensusClientId for [`ParachainConsensusClient`]
pub const PARACHAIN_CONSENSUS_ID: ConsensusClientId = *b"PARA";

/// Slot duration in milliseconds
const SLOT_DURATION: u64 = 12_000;

impl<T> ConsensusClient for ParachainConsensusClient<T>
where
    T: RelayChainOracle + frame_system::Config,
    T::BlockNumber: Into<u32>,
{
    fn verify_consensus(
        &self,
        _host: &dyn ISMPHost,
        state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        let update: ParachainConsensusProof =
            codec::Decode::decode(&mut &proof[..]).map_err(|e| {
                Error::ImplementationSpecific(format!("Cannot decode beacon message: {e}"))
            })?;

        let root = T::state_root(update.relay_height).ok_or_else(|| {
            Error::ImplementationSpecific(format!(
                "Cannot find relay chain height: {}",
                update.relay_height
            ))
        })?;

        let db = StorageProof::new(update.storage_proof).into_memory_db::<BlakeTwo256>();
        let trie = TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &root).build();

        let parachain_heads_key = PARACHAIN_HEADS_KEY.to_vec();

        let mut intermediates = vec![];

        for id in update.para_ids {
            let mut full_key = parachain_heads_key.clone();
            full_key.extend(sp_io::hashing::twox_64(&*id.encode()));
            let header = trie
                .get(&full_key)
                .map_err(|e| {
                    Error::ImplementationSpecific(format!("Error verifying parachain header {e}",))
                })?
                .ok_or_else(|| {
                    Error::ImplementationSpecific(format!(
                        "Cannot find parachain header for ParaId({id})",
                    ))
                })?;

            // ideally all parachain headers are the same
            let header = T::Header::decode(&mut &*header).map_err(|e| {
                Error::ImplementationSpecific(format!("Error decoding parachain header: {e}",))
            })?;

            let (mut timestamp, mut ismp_root) = (0, H256::default());
            for digest in header.digest().logs.iter() {
                match digest {
                    DigestItem::PreRuntime(consensus_engine_id, value)
                        if *consensus_engine_id == AURA_ENGINE_ID =>
                    {
                        let slot = u64::decode(&mut &value[..]).map_err(|e| {
                            Error::ImplementationSpecific(format!(
                                "Cannot decode beacon message: {e}"
                            ))
                        })?;
                        timestamp = Duration::from_millis(slot * SLOT_DURATION).as_secs();
                    }
                    DigestItem::Consensus(consensus_engine_id, value)
                        if *consensus_engine_id == ISMP_ID =>
                    {
                        if value.len() != 32 {
                            Err(Error::ImplementationSpecific(
                                "Header contains an invalid ismp root".into(),
                            ))?
                        }

                        ismp_root = H256::from_slice(&value);
                    }
                    // don't really care about the rest
                    _ => {}
                };
            }

            if timestamp == 0 || ismp_root == H256::default() {
                Err(Error::ImplementationSpecific("Timestamp or ismp root not found".into()))?
            }

            let height: u32 = (*header.number()).into();

            let intermediate = IntermediateState {
                height: StateMachineHeight {
                    id: StateMachineId {
                        state_id: id as u64,
                        consensus_client: PARACHAIN_CONSENSUS_ID,
                    },
                    height: height as u64,
                },
                commitment: StateCommitment {
                    timestamp,
                    ismp_root: Some(ismp_root),
                    state_root: H256::from_slice(header.state_root().as_ref()),
                },
            };

            intermediates.push(intermediate);
        }

        Ok((state, intermediates))
    }

    fn unbonding_period(&self) -> Duration {
        // there's no notion of client expiry, since there's shared security.
        Duration::from_secs(u64::MAX)
    }

    fn verify_membership(
        &self,
        _host: &dyn ISMPHost,
        _item: RequestResponse,
        _root: StateCommitment,
        _proof: &Proof,
    ) -> Result<(), Error> {
        // MerkleProof::new(mmr_size, proof.proof);

        Ok(())
    }

    fn state_trie_key(&self, _request: RequestResponse) -> Vec<u8> {
        todo!()
    }

    fn verify_state_proof(
        &self,
        _host: &dyn ISMPHost,
        key: Vec<u8>,
        root: StateCommitment,
        proof: &Proof,
    ) -> Result<Option<Vec<u8>>, Error> {
        let state_proof: ParachainStateProof = codec::Decode::decode(&mut &*proof.proof)
            .map_err(|e| Error::ImplementationSpecific(format!("failed to decode proof: {e}")))?;

        let db = StorageProof::new(state_proof.storage_proof).into_memory_db::<BlakeTwo256>();

        let data = match state_proof.hasher {
            HashAlgorithm::Keccak => {
                let trie = TrieDBBuilder::<LayoutV0<Keccak256>>::new(&db, &root.state_root).build();
                trie.get(&key).map_err(|e| {
                    Error::ImplementationSpecific(format!("Error reading state proof: {e}"))
                })?
            }
            HashAlgorithm::Blake2 => {
                let trie =
                    TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &root.state_root).build();
                trie.get(&key).map_err(|e| {
                    Error::ImplementationSpecific(format!("Error reading state proof: {e}"))
                })?
            }
        };

        Ok(data)
    }

    fn is_frozen(&self, _: &[u8]) -> Result<(), Error> {
        // parachain consensus client can never be frozen.
        Ok(())
    }
}
