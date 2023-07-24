
//! host functions for light clients

use core::{
    fmt::{Debug},
};
use sp_core::H256;


/// Host functions that allow the light client perform cryptographic operations in native.
pub trait HostFunctions: Clone + Send + Sync + Eq + Debug + Default {
    /// Blake2-256 hashing implementation
    type BlakeTwo256: hash_db::Hasher<Out = H256> + Debug + 'static;
}
