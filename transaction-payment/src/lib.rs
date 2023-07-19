use frame_support::{
    dispatch::{DispatchInfo, DispatchResult, PostDispatchInfo},
    traits::{
        tokens::fungibles::{Credit, Inspect},
        IsSubType, IsType,
    },
};
use log::debug;
use pallet_asset_tx_payment::{Config, InitialPayment, OnChargeAssetTransaction};
use pallet_transaction_payment::OnChargeTransaction;
use scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
    traits::{DispatchInfoOf, Dispatchable, PostDispatchInfoOf, SignedExtension},
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

impl<T: Config> From<ChargeAssetTxPayment<T>> for pallet_asset_tx_payment::ChargeAssetTxPayment<T>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    AssetBalanceOf<T>: Send + Sync + FixedPointOperand,
    BalanceOf<T>: Send + Sync + FixedPointOperand + IsType<ChargeAssetBalanceOf<T>>,
    ChargeAssetIdOf<T>: Send + Sync,
    Credit<T::AccountId, T::Fungibles>: IsType<ChargeAssetLiquidityOf<T>>,
{
    fn from(value: ChargeAssetTxPayment<T>) -> Self {
        Self::from(value.tip, value.asset_id)
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
    T: pallet_ismp::Config,
    T::OnChargeAssetTransaction: OnChargeAssetTransaction<
        T,
        LiquidityInfo = (
            <T::OnChargeAssetTransaction as OnChargeAssetTransaction<T>>::Balance,
            InitialPayment<T>,
        ),
    >,
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
        Option<Self::Call>,
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
        let charge_asset_payment: pallet_asset_tx_payment::ChargeAssetTxPayment<T> =
            self.clone().into();
        if let Ok(valid_transaction) =
            <pallet_asset_tx_payment::ChargeAssetTxPayment<T> as SignedExtension>::validate(
                &charge_asset_payment,
                who,
                call,
                info,
                len,
            )
        {
            return Ok(valid_transaction)
        } else {
            let asset_id = self
                .asset_id
                .ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
            match call.is_sub_type().cloned() {
                Some(pallet_ismp::Call::handle { messages }) => {
                    if let Ok(_) = pallet_ismp::Pallet::<T>::handle_messages(messages) {
                        let fee = pallet_transaction_payment::Pallet::<T>::compute_fee(
                            len as u32, info, self.tip,
                        );
                        if let Ok((_fee, _initial_payment)) = <T::OnChargeAssetTransaction as OnChargeAssetTransaction<T>>::withdraw_fee(
                                    who,
                                    call,
                                    info,
                                    asset_id,
                                    fee.into(),
                                    self.tip.into()
                                )
                            {
                                let priority = ChargeTransactionPayment::<T>::get_priority(
                                    info,
                                    len,
                                    self.tip,
                                    fee,
                                );
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
        let charge_asset_payment: pallet_asset_tx_payment::ChargeAssetTxPayment<T> =
            self.clone().into();
        if let Ok((tip, who, initial_payment, asset_id)) =
            <pallet_asset_tx_payment::ChargeAssetTxPayment<T> as SignedExtension>::pre_dispatch(
                charge_asset_payment,
                who,
                call,
                info,
                len,
            )
        {
            Ok((tip, who.clone(), initial_payment, asset_id, None))
        } else {
            match call.is_sub_type() {
                Some(pallet_ismp::Call::handle { .. }) => {
                    Ok((self.tip, who.clone(), InitialPayment::Nothing, self.asset_id, None))
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
        if let Some((tip, who, initial_payment, asset_id, ismp_call)) = pre {
            match initial_payment {
                InitialPayment::Native(already_withdrawn) => {
                    pallet_transaction_payment::ChargeTransactionPayment::<T>::post_dispatch(
                        Some((tip, who.clone(), already_withdrawn)),
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
                    let (_converted_fee, _converted_tip) =
                        T::OnChargeAssetTransaction::correct_and_deposit_fee(
                            &who,
                            info,
                            post_info,
                            actual_fee.into(),
                            tip.into(),
                            already_withdrawn.into(),
                        )?;
                    debug!(
                        target: "transaction-payment", "{:?} with tip: {:?} paid for asset: {:?}, by account: {}",
                        actual_fee, tip, asset_id.encode(), who
                    );
                }
                InitialPayment::Nothing => {
                    if ismp_call.is_some() {
                        let actual_fee =
                            pallet_transaction_payment::Pallet::<T>::compute_actual_fee(
                                len as u32, info, post_info, tip,
                            );
                        match asset_id {
                            Some(asset_id) => {
                                let _ = <T::OnChargeAssetTransaction as OnChargeAssetTransaction<
                                    T,
                                >>::withdraw_fee(
                                    &who,
                                    &ismp_call.unwrap(),
                                    info,
                                    asset_id,
                                    actual_fee.into(),
                                    tip.into(),
                                );
                            }
                            None => {
                                return Err(TransactionValidityError::Invalid(
                                    InvalidTransaction::Payment,
                                ))
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
