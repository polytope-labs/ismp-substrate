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

//! Pallet primitives
use frame_support::{PalletId, RuntimeDebug};
use ismp_primitives::mmr::{LeafIndex, NodeIndex};
use ismp_rs::{
    consensus::{ConsensusClient, ConsensusClientId},
    module::DispatchResult,
};
use scale_info::TypeInfo;
use sp_core::{crypto::AccountId32, ByteArray, H160};
use sp_std::prelude::*;

/// An MMR proof data for a group of leaves.
#[derive(codec::Encode, codec::Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo)]
pub struct Proof<Hash> {
    /// The indices of the leaves the proof is for.
    pub leaf_indices: Vec<LeafIndex>,
    /// Number of leaves in MMR, when the proof was generated.
    pub leaf_count: NodeIndex,
    /// Proof elements (hashes of siblings of inner nodes on the path to the leaf).
    pub items: Vec<Hash>,
}

/// Merkle Mountain Range operation error.
#[derive(RuntimeDebug, codec::Encode, codec::Decode, PartialEq, Eq, scale_info::TypeInfo)]
#[allow(missing_docs)]
pub enum Error {
    InvalidNumericOp,
    Push,
    GetRoot,
    Commit,
    GenerateProof,
    Verify,
    LeafNotFound,
    PalletNotIncluded,
    InvalidLeafIndex,
    InvalidBestKnownBlock,
}

/// A trait that returns a reference to a consensus client based on its Id
/// This trait should be implemented in the runtime
pub trait ConsensusClientProvider {
    /// Returns a reference to a consensus client
    fn consensus_client(
        id: ConsensusClientId,
    ) -> Result<Box<dyn ConsensusClient>, ismp_rs::error::Error>;
}

/// Module identification types supported by ismp
#[derive(PartialEq, Eq, scale_info::TypeInfo)]
pub enum ModuleId {
    /// Unique Pallet identification in runtime
    Pallet(PalletId),
    /// Contract account id
    Contract(AccountId32),
    /// Evm contract
    Evm(H160),
}
impl ModuleId {
    /// Convert module id to raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            ModuleId::Pallet(pallet_id) => pallet_id.0.to_vec(),
            ModuleId::Contract(account_id) => account_id.to_raw_vec(),
            ModuleId::Evm(account_id) => account_id.0.to_vec(),
        }
    }

    /// Derive module id from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() == 8 {
            let mut inner = [0u8; 8];
            inner.copy_from_slice(bytes);
            Ok(Self::Pallet(PalletId(inner)))
        } else if bytes.len() == 32 {
            Ok(Self::Contract(AccountId32::from_slice(bytes).expect("Infallible")))
        } else if bytes.len() == 20 {
            Ok(Self::Evm(H160::from_slice(bytes)))
        } else {
            Err("Unknown Module ID format")
        }
    }
}

/// Creates a distinction between the types of gas
pub enum GasType {
    /// EVM gas consumption
    Evm {
        /// Gas used in executing
        gas_used: u64,
        /// Gas limit provided
        gas_limit: u64,
    },
    /// Ink gas consumption
    Ink {
        /// Gas used in executing
        gas_used: u64,
        /// Gas limit provided
        gas_limit: u64,
    },
}

/// A helper function that accumulates the total gas used and total gas limit from a slice of module
/// dispatch results It return results for both evm and ink
pub fn extract_total_gas(
    res: &[DispatchResult],
    evm_used_total: u64,
    evm_limit_total: u64,
    ink_used_total: u64,
    ink_limit_total: u64,
) -> ((u64, u64), (u64, u64)) {
    let gas = res
        .iter()
        .filter_map(|res| match res {
            Ok(success) => {
                if success.gas.gas_used.is_some() && success.gas.gas_limit.is_some() {
                    let module_id = ModuleId::from_bytes(&success.module_id).ok()?;
                    match module_id {
                        ModuleId::Pallet(_) => None,
                        ModuleId::Contract(_) => Some(GasType::Ink {
                            gas_used: success.gas.gas_used.expect("Infallible"),
                            gas_limit: success.gas.gas_limit.expect("Infallible"),
                        }),
                        ModuleId::Evm(_) => Some(GasType::Evm {
                            gas_used: success.gas.gas_used.expect("Infallible"),
                            gas_limit: success.gas.gas_limit.expect("Infallible"),
                        }),
                    }
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect::<Vec<GasType>>();

    let (mut evm_gas_used, mut evm_gas_limit, mut ink_gas_used, mut ink_gas_limit) = (0, 0, 0, 0);
    for gas_type in gas {
        match gas_type {
            GasType::Ink { gas_used, gas_limit } => {
                ink_gas_used += gas_used;
                ink_gas_limit += gas_limit;
            }
            GasType::Evm { gas_used, gas_limit } => {
                evm_gas_used += gas_used;
                evm_gas_limit += gas_limit;
            }
        }
    }

    (
        (evm_gas_used + evm_used_total, evm_gas_limit + evm_limit_total),
        (ink_gas_used + ink_used_total, ink_gas_limit + ink_limit_total),
    )
}
