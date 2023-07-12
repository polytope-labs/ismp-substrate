use frame_support::{
    dispatch::{DispatchInfo, DispatchResult, PostDispatchInfo},
    traits::{
        tokens::fungibles::{Credit, Inspect},
        IsSubType, IsType,
    },
};
use scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
    traits::{DispatchInfoOf, Dispatchable, PostDispatchInfoOf, SignedExtension, Zero},
    transaction_validity::{
        InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction,
    },
    FixedPointOperand,
};
// use pallet_ismp::pallet;
use pallet_asset_tx_payment::InitialPayment;
use pallet_transaction_payment::OnChargeTransaction;
use payment::OnChargeAssetTransaction;

mod payment;
// Type aliases used for interaction with `OnChargeTransaction`.
pub(crate) type OnChargeTransactionOf<T> =
    <T as pallet_transaction_payment::Config>::OnChargeTransaction;
// Balance type alias.
pub(crate) type BalanceOf<T> = <OnChargeTransactionOf<T> as OnChargeTransaction<T>>::Balance;

// Type alias used for interaction with fungibles (assets).
// Balance type alias.
pub(crate) type AssetBalanceOf<T> =
    <<T as pallet_asset_tx_payment::Config>::Fungibles as Inspect<
        <T as frame_system::Config>::AccountId,
    >>::Balance;

// Type aliases used for interaction with `OnChargeAssetTransaction`.
// Balance type alias.
pub(crate) type ChargeAssetBalanceOf<T> =
    <<T as Config>::OnChargeAssetTransaction as OnChargeAssetTransaction<T>>::Balance;
// Asset id type alias.
pub(crate) type ChargeAssetIdOf<T> =
    <<T as Config>::OnChargeAssetTransaction as OnChargeAssetTransaction<T>>::AssetId;
// Liquity info type alias.
pub(crate) type ChargeAssetLiquidityOf<T> =
    <<T as Config>::OnChargeAssetTransaction as OnChargeAssetTransaction<T>>::LiquidityInfo;

// Type to track whether call is ISMP call or not
pub(crate) type IsIsmpCall = bool;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_ismp::Config + pallet_asset_tx_payment::pallet::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The fungibles instance used to pay for transactions in assets.
        // type Fungibles: Balanced<Self::AccountId>;
        /// The actual transaction charging logic that charges the fees.
        type OnChargeAssetTransaction: OnChargeAssetTransaction<Self>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A transaction fee `actual_fee`, of which `tip` was added to the minimum inclusion fee,
        /// has been paid by `who` in an asset `asset_id`.
        AssetTxFeePaid {
            who: T::AccountId,
            actual_fee: AssetBalanceOf<T>,
            tip: AssetBalanceOf<T>,
            asset_id: Option<ChargeAssetIdOf<T>>,
        },
    }
}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue. Allows paying via both `Currency` as well as `fungibles::Balanced`.
///
/// Wraps the transaction logic in [`pallet_transaction_payment`] and extends it with assets.
/// An asset id of `None` falls back to the underlying transaction payment via the native currency.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ChargeAssetTxPayment<T: Config> {
    #[codec(compact)]
    tip: BalanceOf<T>,
    asset_id: Option<ChargeAssetIdOf<T>>,
}

impl<T: Config> ChargeAssetTxPayment<T>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    AssetBalanceOf<T>: Send + Sync + FixedPointOperand,
    BalanceOf<T>: Send + Sync + FixedPointOperand + IsType<ChargeAssetBalanceOf<T>>,
    ChargeAssetIdOf<T>: Send + Sync,
    Credit<T::AccountId, T::Fungibles>: IsType<ChargeAssetLiquidityOf<T>>,
{
    /// Utility constructor. Used only in client/factory code.
    pub fn from(tip: BalanceOf<T>, asset_id: Option<ChargeAssetIdOf<T>>) -> Self {
        Self { tip, asset_id }
    }

    /// Fee withdrawal logic that dispatches to either `OnChargeAssetTransaction` or
    /// `OnChargeTransaction`.
    fn withdraw_fee(
        &self,
        who: &T::AccountId,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
    ) -> Result<(BalanceOf<T>, InitialPayment<T>), TransactionValidityError> {
        let fee = pallet_transaction_payment::Pallet::<T>::compute_fee(len as u32, info, self.tip);
        debug_assert!(self.tip <= fee, "tip should be included in the computed fee");
        if fee.is_zero() {
            Ok((fee, InitialPayment::Nothing))
        } else if let Some(asset_id) = self.asset_id {
            <T as pallet::Config>::OnChargeAssetTransaction::withdraw_fee(
                who,
                call,
                info,
                asset_id,
                fee.into(),
                self.tip.into(),
            )
            .map(|i| (fee, InitialPayment::Asset(i.into())))
        } else {
            <OnChargeTransactionOf<T> as OnChargeTransaction<T>>::withdraw_fee(
                who, call, info, fee, self.tip,
            )
            .map(|i| (fee, InitialPayment::Native(i)))
            .map_err(|_| -> TransactionValidityError { InvalidTransaction::Payment.into() })
        }
    }
}

impl<T: Config> sp_std::fmt::Debug for ChargeAssetTxPayment<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "ChargeAssetTxPayment<{:?}, {:?}>", self.tip, self.asset_id.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Config> SignedExtension for ChargeAssetTxPayment<T>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>
        + IsSubType<pallet_ismp::Call<T>>,
    T::Hash: From<H256>,
    AssetBalanceOf<T>: Send + Sync + FixedPointOperand,
    BalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand + IsType<ChargeAssetBalanceOf<T>>,
    ChargeAssetIdOf<T>: Send + Sync,
    Credit<T::AccountId, <T as pallet_asset_tx_payment::Config>::Fungibles>:
        IsType<ChargeAssetLiquidityOf<T>>,
{
    const IDENTIFIER: &'static str = "ChargeAssetTxPayment";
    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = (
        // tip
        BalanceOf<T>,
        // who paid the fee
        Self::AccountId,
        // imbalance resulting from withdrawing the fee
        InitialPayment<T>,
        // asset_id for the transaction payment
        Option<ChargeAssetIdOf<T>>,
        // boolean to indicate whether the call is an ISMP call
        IsIsmpCall,
    );

    fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
        Ok(())
    }

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> TransactionValidity {
        use pallet_transaction_payment::ChargeTransactionPayment;
        let (fee, _) = self.withdraw_fee(who, call, info, len)?;
        let priority = ChargeTransactionPayment::<T>::get_priority(info, len, self.tip, fee);
        Ok(ValidTransaction { priority, ..Default::default() })
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        if let Ok((_fee, initial_payment)) = self.withdraw_fee(who, call, info, len) {
            Ok((self.tip, who.clone(), initial_payment, self.asset_id, false))
        } else {
            match call.is_sub_type().cloned() {
                Some(pallet_ismp::Call::handle { messages }) => {
                    if let Ok(_) = pallet_ismp::Pallet::<T>::handle_messages(messages) {
                        ()
                    } else {
                        return Err(TransactionValidityError::Invalid(InvalidTransaction::Payment))
                    }
                    if let Ok((_fee, initial_payment)) = self.withdraw_fee(who, call, info, len) {
                        // Self::CallType::ISMPCall
                        Ok((self.tip, who.clone(), initial_payment, self.asset_id, true))
                    } else {
                        Err(TransactionValidityError::Invalid(InvalidTransaction::Payment))
                    }
                }
                _ => Err(TransactionValidityError::Invalid(InvalidTransaction::Payment)),
            }
        }
    }

    fn post_dispatch(
        pre: Option<Self::Pre>,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &PostDispatchInfoOf<Self::Call>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        if let Some((tip, who, initial_payment, asset_id, is_ismp_call)) = pre {
            // if ISMP call, withdraw fee
            if is_ismp_call {
                // withdraw fee
            }
            match initial_payment {
                InitialPayment::Native(already_withdrawn) => {
                    pallet_transaction_payment::ChargeTransactionPayment::<T>::post_dispatch(
                        Some((tip, who, already_withdrawn)),
                        info,
                        post_info,
                        len,
                        result,
                    )?;
                }
                InitialPayment::Asset(already_withdrawn) => {
                    let actual_fee = pallet_transaction_payment::Pallet::<T>::compute_actual_fee(
                        len as u32, info, post_info, tip,
                    );

                    let (converted_fee, converted_tip) =
                        <T as pallet::Config>::OnChargeAssetTransaction::correct_and_deposit_fee(
                            &who,
                            info,
                            post_info,
                            actual_fee.into(),
                            tip.into(),
                            already_withdrawn.into(),
                        )?;
                    Pallet::<T>::deposit_event(Event::<T>::AssetTxFeePaid {
                        who,
                        actual_fee: converted_fee,
                        tip: converted_tip,
                        asset_id,
                    });
                }
                InitialPayment::Nothing => {
                    // `actual_fee` should be zero here for any signed extrinsic. It would be
                    // non-zero here in case of unsigned extrinsics as they don't pay fees but
                    // `compute_actual_fee` is not aware of them. In both cases it's fine to just
                    // move ahead without adjusting the fee, though, so we do nothing.
                    debug_assert!(tip.is_zero(), "tip should be zero if initial fee was zero.");
                }
            }
        }

        Ok(())
    }
}
