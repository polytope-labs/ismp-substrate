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

//! ISMP Parachain Consensus Inherent Provider
//!
//! This exports the inherent provider for including ISMP parachain consensus updates as block
//! inherents.

use codec::Encode;
use cumulus_primitives_core::PersistedValidationData;
use cumulus_relay_chain_interface::{PHash, RelayChainInterface};
use ismp::messaging::ConsensusMessage;
use ismp_parachain::consensus::{parachain_header_storage_key, ParachainConsensusProof};

pub struct ParachainConsensusProvider(ConsensusMessage);

impl ParachainConsensusProvider {
    /// Create the [`ParachainConsensusProvider`] at the given `relay_parent`.
    pub async fn create_at(
        relay_parent: PHash,
        relay_chain_interface: &impl RelayChainInterface,
        validation_data: PersistedValidationData,
    ) -> Result<ParachainConsensusProvider, anyhow::Error> {
        // todo: read the para_ids from the runtime.
        let para_ids = vec![];

        let keys = para_ids.iter().map(|id| parachain_header_storage_key(*id).0).collect();
        let storage_proof = relay_chain_interface
            .prove_read(relay_parent, &keys)
            .await?
            .into_iter_nodes()
            .collect();

        let consensus_proof = ParachainConsensusProof {
            para_ids,
            relay_height: validation_data.relay_parent_number,
            storage_proof,
        };
        let message = ConsensusMessage {
            consensus_client_id: ismp_parachain::consensus::PARACHAIN_CONSENSUS_ID,
            consensus_proof: consensus_proof.encode(),
        };

        Ok(ParachainConsensusProvider(message))
    }
}

#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for ParachainConsensusProvider {
    async fn provide_inherent_data(
        &self,
        inherent_data: &mut sp_inherents::InherentData,
    ) -> Result<(), sp_inherents::Error> {
        inherent_data.put_data(ismp_parachain::INHERENT_IDENTIFIER, &self.0)
    }

    async fn try_handle_error(
        &self,
        _: &sp_inherents::InherentIdentifier,
        _: &[u8],
    ) -> Option<Result<(), sp_inherents::Error>> {
        None
    }
}
