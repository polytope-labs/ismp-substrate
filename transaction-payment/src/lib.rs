use frame_support::{
    dispatch::{DispatchInfo, DispatchResult, PostDispatchInfo},
    traits::{
        tokens::fungibles::{Credit, Inspect},
        IsSubType, IsType,
    },
};
use pallet_asset_tx_payment::{Config, Event, InitialPayment, OnChargeAssetTransaction, Pallet};
use pallet_transaction_payment::OnChargeTransaction;
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

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ChargeAssetTxPayment<T: Config> {
    inner: pallet_asset_tx_payment::ChargeAssetTxPayment<T>,
}

impl<T: Config> sp_std::fmt::Debug for ChargeAssetTxPayment<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        <pallet_asset_tx_payment::ChargeAssetTxPayment<T> as sp_std::fmt::Debug>::fmt(
            &self.inner,
            f,
        )
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: Config> SignedExtension for ChargeAssetTxPayment<T>
where
    T: pallet_ismp::Config,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>
        + IsSubType<pallet_ismp::Call<T>>
        + pallet_ismp::Config,
    T::Hash: From<H256>,
    AssetBalanceOf<T>: Send + Sync + FixedPointOperand,
    BalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand + IsType<ChargeAssetBalanceOf<T>>,
    ChargeAssetIdOf<T>: Send + Sync,
    Credit<T::AccountId, T::Fungibles>: IsType<ChargeAssetLiquidityOf<T>>,
{
    const IDENTIFIER: &'static str = "IsmpAssetTxPayment";
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
        Option<IsIsmpCall>,
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
        if let Ok(valid_transaction) =
            <pallet_asset_tx_payment::ChargeAssetTxPayment<T> as SignedExtension>::validate(
                &self.inner,
                who,
                call,
                info,
                len,
            )
        {
            return Ok(valid_transaction)
        } else {
            match call.is_sub_type().cloned() {
                Some(pallet_ismp::Call::handle { messages }) => {
                    if let Ok(_) = pallet_ismp::Pallet::<T>::handle_messages(messages) {
                        // if let Ok((fee, _initial_payment)) = self.inner.withdraw_fee(who, call,
                        // info, len)
                        if let Ok((fee, _initial_payment)) =
                            <pallet_asset_tx_payment::ChargeAssetTxPayment<T>>::withdraw_fee(
                                &self.inner,
                                who,
                                call,
                                info,
                                len,
                            )
                        {
                            let priority = ChargeTransactionPayment::<T>::get_priority(
                                info,
                                len,
                                self.inner.tip,
                                fee,
                            );
                            // let priority = <pallet_asset_tx_payment::ChargeAssetTxPayment<T> as
                            // SignedExtension>::<T>::get_priority(
                            //     info,
                            //     len,
                            //     self.inner.tip,
                            //     fee,
                            // );
                            Ok(ValidTransaction { priority, ..Default::default() })
                        } else {
                            Err(TransactionValidityError::Invalid(InvalidTransaction::Payment))
                        }
                    } else {
                        return Err(TransactionValidityError::Invalid(InvalidTransaction::Payment))
                    }
                }
                _ => Err(TransactionValidityError::Invalid(InvalidTransaction::Payment)),
            }
        }
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        let (tip, who, initial_payment, asset_id) =
            <pallet_asset_tx_payment::ChargeAssetTxPayment<T> as SignedExtension>::pre_dispatch(
                self.inner, who, call, info, len,
            )?;
        Ok((tip, who.clone(), initial_payment, asset_id, None))
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
            if is_ismp_call.is_some() {
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
                        T::OnChargeAssetTransaction::correct_and_deposit_fee(
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
