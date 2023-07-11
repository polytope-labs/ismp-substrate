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

//! host functions for light clients

use core::fmt::Debug;
use sp_core::H256;

/// Host functions that allow the light client perform cryptographic operations in native.
pub trait HostFunctions: Clone + Send + Sync + Eq + Debug + Default {
    /// Blake2-256 hashing implementation
    type BlakeTwo256: hash_db::Hasher<Out = H256> + Debug + 'static;
}
