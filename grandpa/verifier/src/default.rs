use sp_core::{crypto::AccountId32, H256};
use subxt::config::polkadot::PolkadotExtrinsicParams as ParachainExtrinsicParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultConfig;

impl subxt::config::Config for DefaultConfig {
    type Index = u32;
    type Hash = H256;
    type AccountId = AccountId32;
    type Address = sp_runtime::MultiAddress<Self::AccountId, u32>;
    type Signature = sp_runtime::MultiSignature;
    type Hasher = subxt::config::substrate::BlakeTwo256;
    type Header =
        subxt::config::substrate::SubstrateHeader<u32, subxt::config::substrate::BlakeTwo256>;
    type ExtrinsicParams = ParachainExtrinsicParams<Self>;
}
