use primitives::{FinalityProof, ParachainHeaderProofs};
use alloc::{collections::BTreeMap, vec::Vec};
use codec::{Decode, Encode};
use codec::alloc::collections::BTreeMap;
use sp_core::H256;
use sp_runtime::traits::BlakeTwo256;

/// Relay chain substrate header type
pub type SubstrateHeader = sp_runtime::generic::Header<u32, BlakeTwo256>;

/// Parachain headers with a Grandpa finality proof.
#[derive(Clone, Debug, Encode, Decode,)]
pub struct Header {
    /// The grandpa finality proof: contains relay chain headers from the
    /// last known finalized grandpa block.
    pub finality_proof: FinalityProof<SubstrateHeader>,
    /// Contains a map of relay chain header hashes to parachain headers
    /// finalzed at the relay chain height. We check for this parachain header finalization
    /// via state proofs. Also contains extrinsic proof for timestamp.
    pub parachain_headers: Option<BTreeMap<H256, ParachainHeaderProofs>>,
}

/// [`ClientMessage`] definition
#[derive(Clone, Debug, Encode, Decode,)]
pub enum ClientMessage {
    /// This is the variant representing the standalone chain
    StandaloneChainMessage(StandaloneChainMessage),
    /// This is the variant representing the relay chain
    RelayChainMessage(RelayChainMessage),
}

#[derive(Clone, Debug, Encode, Decode,)]
pub struct StandaloneChainMessage {
    /// finality proof
    pub finality_proof: FinalityProof<SubstrateHeader>,
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct RelayChainMessage {
    /// finality proof
    pub finality_proof: FinalityProof<SubstrateHeader>,
    /// parachain headers
    pub parachain_headers: BTreeMap<H256, ParachainHeaderProofs>,
}

