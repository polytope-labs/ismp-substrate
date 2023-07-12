use super::AssetBalanceOf;
use frame_support::{traits::tokens::Balance, unsigned::TransactionValidityError};
use pallet_ismp::Config;
use scale_codec::FullCodec;
use scale_info::TypeInfo;
use sp_runtime::traits::{DispatchInfoOf, MaybeSerializeDeserialize, PostDispatchInfoOf};
use sp_std::fmt::Debug;

/// Handle withdrawing, refunding and depositing of transaction fees.
pub trait OnChargeAssetTransaction<T: Config + pallet_asset_tx_payment::Config> {
    /// The underlying integer type in which fees are calculated.
    type Balance: Balance;
    /// The type used to identify the assets used for transaction payment.
    type AssetId: FullCodec + Copy + MaybeSerializeDeserialize + Debug + Default + Eq + TypeInfo;
    /// The type used to store the intermediate values between pre- and post-dispatch.
    type LiquidityInfo;

    /// Before the transaction is executed the payment of the transaction fees needs to be secured.
    ///
    /// Note: The `fee` already includes the `tip`.
    fn withdraw_fee(
        who: &T::AccountId,
        call: &T::RuntimeCall,
        dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
        asset_id: Self::AssetId,
        fee: Self::Balance,
        tip: Self::Balance,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError>;

    /// After the transaction was executed the actual fee can be calculated.
    /// This function should refund any overpaid fees and optionally deposit
    /// the corrected amount.
    ///
    /// Note: The `fee` already includes the `tip`.
    ///
    /// Returns the fee and tip in the asset used for payment as (fee, tip).
    fn correct_and_deposit_fee(
        who: &T::AccountId,
        dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
        post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        corrected_fee: Self::Balance,
        tip: Self::Balance,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(AssetBalanceOf<T>, AssetBalanceOf<T>), TransactionValidityError>;
}
