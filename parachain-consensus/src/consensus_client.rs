use crate::RelayChainOracle;
use codec::{Decode, Encode};
use hex_literal::hex;
use ismp::{
    consensus_client::{
        ConsensusClient, IntermediateState, StateCommitment, StateMachineHeight, StateMachineId,
    },
    error::Error,
    host::ISMPHost,
    messaging::Proof,
    router::RequestResponse,
};
use sp_runtime::traits::{BlakeTwo256, Header};
use sp_trie::{LayoutV0, StorageProof, Trie, TrieDBBuilder};
use std::{marker::PhantomData, time::Duration};

struct ParachainConsensusClient<T>(PhantomData<T>);

/// Information necessary to prove the sibling parachain's finalization to this
/// parachain.
#[derive(Encode, Decode)]
pub struct ParachainConsensusUpdate {
    /// List of para ids contained in the proof
    pub para_ids: Vec<u32>,
    /// Height of the relay chain for the given proof
    pub relay_height: u32,
    /// Storage proof for the parachain headers
    pub storage_proof: Vec<Vec<u8>>,
}

/// Static key for parachain headers in the relay chain storage
const PARACHAIN_HEADS_KEY: [u8; 32] =
    hex!("cd710b30bd2eab0352ddcc26417aa1941b3c252fcb29d88eff4f3de5de4476c3");

impl<T> ConsensusClient for ParachainConsensusClient<T>
where
    T: RelayChainOracle + frame_system::Config,
    T::BlockNumber: Into<u32>,
{
    fn verify_consensus(
        &self,
        _host: &dyn ISMPHost,
        _state: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<IntermediateState>), Error> {
        let update: ParachainConsensusUpdate =
            codec::Decode::decode(&mut &proof[..]).map_err(|e| {
                Error::ImplementationSpecific(format!("Cannot decode beacon message: {e}"))
            })?;

        let root = T::storage_root(update.height).ok_or_else(|| {
            Error::ImplementationSpecific(format!(
                "Cannot find relay chain height: {}",
                update.height
            ))
        })?;

        let db = StorageProof::new(update.proof).into_memory_db::<BlakeTwo256>();
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

            let header = T::Header::decode(&mut &*header).map_err(|e| {
                Error::ImplementationSpecific(format!("Error decoding parachain header",))
            })?;

            let digests = header.digest().logs.iter().fold();

            let intermediate = IntermediateState {
                height: StateMachineHeight {
                    id: StateMachineId { state_id: id as u64, consensus_client: 0 },
                    height: header.number().into() as u64,
                },
                commitment: StateCommitment { timestamp: 0, ismp_root: None, state_root: header.state_root().as_ref() },
            };

            intermediates.push(intermediate);
        }

        todo!()
    }

    fn unbonding_period(&self) -> Duration {
        // there's no notion of client expiry, since there's shared security.
        Duration::from_secs(u64::MAX)
    }

    fn verify_membership(
        &self,
        host: &dyn ISMPHost,
        item: RequestResponse,
        root: StateCommitment,
        proof: &Proof,
    ) -> Result<(), Error> {
        todo!()
    }

    fn verify_state_proof(
        &self,
        host: &dyn ISMPHost,
        key: Vec<u8>,
        root: StateCommitment,
        proof: &Proof,
    ) -> Result<Vec<u8>, Error> {
        todo!()
    }

    fn verify_non_membership(
        &self,
        host: &dyn ISMPHost,
        item: RequestResponse,
        root: StateCommitment,
        proof: &Proof,
    ) -> Result<(), Error> {
        todo!()
    }

    fn is_frozen(&self, trusted_consensus_state: &[u8]) -> Result<(), Error> {
        Ok(())
    }
}
