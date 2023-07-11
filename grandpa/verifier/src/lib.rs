// Copyright (C) 2023 Polytope Labs.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! GRANDPA consensus client verification function

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::all)]
#![deny(missing_docs)]

mod state_machine;

extern crate alloc;

use alloc::vec;
use anyhow::anyhow;
use codec::{Decode, Encode};
use finality_grandpa::Chain;
use hash_db::Hasher;
use primitives::{error, justification::{find_scheduled_change, AncestryChain, GrandpaJustification}, parachain_header_storage_key, ConsensusState, HostFunctions, ParachainHeaderProofs, ParachainHeadersWithFinalityProof, FinalityProof};
use sp_core::H256;
use sp_runtime::traits::Header;
use sp_trie::{LayoutV0, StorageProof};

/// This function verifies the GRANDPA finality proof for both standalone chain and parachain headers.
///
/// TODO: return verified header and the associated time stamp
pub fn verify_grandpa_finality_proof<H, Host>(
    mut consensus_state: ConsensusState,
    finality_proof: FinalityProof<H>
) -> Result<(ConsensusState, Header), error::Error>
    where
        H: Header<Hash = H256, Number = u32>,
        H::Number: finality_grandpa::BlockNumberOps + Into<u32>,
        Host: HostFunctions,
        Host::BlakeTwo256: Hasher<Out = H256>,
{
    // First validate unknown headers.
    let headers = AncestryChain::<H>::new(&finality_proof.unknown_headers);

    let target = finality_proof
        .unknown_headers
        .iter()
        .max_by_key(|h| *h.number())
        .ok_or_else(|| anyhow!("Unknown headers can't be empty!"))?;

    // this is illegal
    if target.hash() != finality_proof.block {
        Err(anyhow!("Latest finalized block should be highest block in unknown_headers"))?;
    }

    let justification = GrandpaJustification::<H>::decode(&mut &finality_proof.justification[..])?;

    if justification.commit.target_hash != finality_proof.block {
        Err(anyhow!("Justification target hash and finality proof block hash mismatch"))?;
    }

    let from = consensus_state.latest_hash;

    let base = finality_proof
        .unknown_headers
        .iter()
        .min_by_key(|h| *h.number())
        .ok_or_else(|| anyhow!("Unknown headers can't be empty!"))?;

    if base.number() < &consensus_state.latest_height {
        headers.ancestry(base.hash(), consensus_state.latest_hash).map_err(|_| {
            anyhow!(
				"[verify_grandpa_finality_proof] Invalid ancestry (base -> latest relay block)!"
			)
        })?;
    }

    let mut finalized = headers.ancestry(from, target.hash()).map_err(|_| {
        anyhow!("[verify_grandpa_finality_proof] Invalid ancestry!")
    })?;
    finalized.sort();

    // 2. verify justification.
    justification.verify::<Host>(consensus_state.current_set_id, &consensus_state.current_authorities)?;

    Ok((consensus_state, base))
}
/// This function verifies the GRANDPA finality proof for relay chain headers.
///
/// Next, we prove the finality of parachain headers, by verifying patricia-merkle trie state proofs
/// of these headers, stored at the recently finalized relay chain heights.
/// TODO: return verified headers(compared with the consensus state para ids) and the associated time stamp
/// TODO: remove all host functions
pub fn verify_parachain_headers_with_grandpa_finality_proof<H, Host>(
    mut consensus_state: ConsensusState,
    proof: ParachainHeadersWithFinalityProof<H>,
) -> Result<(ConsensusState, Vec<Header>), error::Error>
    where
        H: Header<Hash = H256, Number = u32>,
        H::Number: finality_grandpa::BlockNumberOps + Into<u32>,
        Host: HostFunctions,
        Host::BlakeTwo256: Hasher<Out = H256>,
{
    let (mut consensus_state, base_header) = verify_grandpa_finality_proof(consensus_state, finality_proof)?;
    let ParachainHeadersWithFinalityProof { finality_proof, parachain_headers } = proof;

    // verifies state proofs of parachain headers in finalized relay chain headers.
    let mut para_heights = vec![];
    let mut verified_parachain_headers = vec![];
    for (hash, proofs) in parachain_headers {
        if finalized.binary_search(&hash).is_err() {
            // seems relay hash isn't in the finalized chain.
            continue
        }
        let relay_chain_header =
            headers.header(&hash).expect("Headers have been checked by AncestryChain; qed");

        let ParachainHeaderProofs { extrinsic_proof, extrinsic, state_proof, para_id } = proofs;
        let proof = StorageProof::new(state_proof);
        let key = parachain_header_storage_key(para_id);
        // verify patricia-merkle state proofs
        let header = state_machine::read_proof_check::<Host::BlakeTwo256, _>(
            relay_chain_header.state_root(),
            proof,
            &[key.as_ref()],
        )
            .map_err(|err| anyhow!("error verifying parachain header state proof: {err}"))?
            .remove(key.as_ref())
            .flatten()
            .ok_or_else(|| anyhow!("Invalid proof, parachain header not found"))?;
        let parachain_header = H::decode(&mut &header[..])?;
        let para_height = parachain_header.number().clone().into();
        para_heights.push(para_height);
        // Timestamp extrinsic should be the first inherent and hence the first extrinsic
        // https://github.com/paritytech/substrate/blob/d602397a0bbb24b5d627795b797259a44a5e29e9/primitives/trie/src/lib.rs#L99-L101
        let key = codec::Compact(0u32).encode();
        // verify extrinsic proof for timestamp extrinsic
        sp_trie::verify_trie_proof::<LayoutV0<Host::BlakeTwo256>, _, _, _>(
            parachain_header.extrinsics_root(),
            &extrinsic_proof,
            &vec![(key, Some(&extrinsic[..]))],
        )
            .map_err(|_| anyhow!("Invalid extrinsic proof"))?;

        if consensus_state.latest_para_heights.contains_key(&para_height) {
            verified_parachain_headers.push(parachain_header);
        }

    }

    // SetS new consensus state, optionally rotating authorities
    consensus_state.latest_hash = target.hash();
    consensus_state.latest_height = (*target.number()).into();
    if let Some(max_height) = para_heights.into_iter().max() {
        consensus_state.latest_height = max_height;
    }
    if let Some(scheduled_change) = find_scheduled_change::<H>(&target) {
        consensus_state.current_set_id += 1;
        consensus_state.current_authorities = scheduled_change.next_authorities;
    }

    Ok((consensus_state, verified_parachain_headers))
}
