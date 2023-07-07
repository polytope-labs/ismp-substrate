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

// use crate::Config;

use codec::{Decode, Encode, MaxEncodedLen};
use sp_runtime::{
    traits::{DispatchInfoOf, PostDispatchInfoOf, Saturating, Zero},
    transaction_validity::InvalidTransaction,
};
use sp_std::marker::PhantomData;

use frame_support::{
    traits::{Currency, ExistenceRequirement, Imbalance, OnUnbalanced, WithdrawReasons},
    unsigned::TransactionValidityError,
};
use frame_system::Config;
use scale_info::TypeInfo;

type NegativeImbalanceOf<C, T> =
    <C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// Handle withdrawing, refunding and depositing of transaction fees.
pub trait OnChargeTransaction<T: Config> {
    /// The underlying integer type in which fees are calculated.
    type Balance: frame_support::traits::tokens::Balance;

    type LiquidityInfo: Default;

    /// Before the transaction is executed the payment of the transaction fees
    /// need to be secured.
    ///
    /// Note: The `fee` already includes the `tip`.
    fn withdraw_fee(
        who: &T::AccountId,
        call: &T::RuntimeCall,
        dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
        fee: Self::Balance,
        tip: Self::Balance,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError>;

    /// After the transaction was executed the actual fee can be calculated.
    /// This function should refund any overpaid fees and optionally deposit
    /// the corrected amount.
    ///
    /// Note: The `fee` already includes the `tip`.
    fn correct_and_deposit_fee(
        who: &T::AccountId,
        dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
        post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        corrected_fee: Self::Balance,
        tip: Self::Balance,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(), TransactionValidityError>;
}

/// Implements the transaction payment for a pallet implementing the `Currency`
/// trait (eg. the pallet_balances) using an unbalance handler (implementing
/// `OnUnbalanced`).
///
/// The unbalance handler is given 2 unbalanceds in [`OnUnbalanced::on_unbalanceds`]: fee and
/// then tip.
pub struct CurrencyAdapter<C, OU>(PhantomData<(C, OU)>);

/// Default implementation for a Currency and an OnUnbalanced handler.
///
/// The unbalance handler is given 2 unbalanceds in [`OnUnbalanced::on_unbalanceds`]: fee and
/// then tip.
impl<T, C, OU> OnChargeTransaction<T> for CurrencyAdapter<C, OU>
where
    T: Config,
    C: Currency<<T as frame_system::Config>::AccountId>,
    C::PositiveImbalance: Imbalance<
        <C as Currency<<T as frame_system::Config>::AccountId>>::Balance,
        Opposite = C::NegativeImbalance,
    >,
    C::NegativeImbalance: Imbalance<
        <C as Currency<<T as frame_system::Config>::AccountId>>::Balance,
        Opposite = C::PositiveImbalance,
    >,
    OU: OnUnbalanced<NegativeImbalanceOf<C, T>>,
{
    type LiquidityInfo = Option<NegativeImbalanceOf<C, T>>;
    type Balance = <C as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Withdraw the predicted fee from the transaction origin.
    ///
    /// Note: The `fee` already includes the `tip`.
    fn withdraw_fee(
        who: &T::AccountId,
        _call: &T::RuntimeCall,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        fee: Self::Balance,
        tip: Self::Balance,
    ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
        if fee.is_zero() {
            return Ok(None)
        }

        let withdraw_reason = if tip.is_zero() {
            WithdrawReasons::TRANSACTION_PAYMENT
        } else {
            WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
        };

        match C::withdraw(who, fee, withdraw_reason, ExistenceRequirement::KeepAlive) {
            Ok(imbalance) => Ok(Some(imbalance)),
            Err(_) => Err(InvalidTransaction::Payment.into()),
        }
    }

    /// Hand the fee and the tip over to the `[OnUnbalanced]` implementation.
    /// Since the predicted fee might have been too high, parts of the fee may
    /// be refunded.
    ///
    /// Note: The `corrected_fee` already includes the `tip`.
    fn correct_and_deposit_fee(
        who: &T::AccountId,
        _dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
        _post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        corrected_fee: Self::Balance,
        tip: Self::Balance,
        already_withdrawn: Self::LiquidityInfo,
    ) -> Result<(), TransactionValidityError> {
        if let Some(paid) = already_withdrawn {
            // Calculate how much refund we should return
            let refund_amount = paid.peek().saturating_sub(corrected_fee);
            // refund to the the account that paid the fees. If this fails, the
            // account might have dropped below the existential balance. In
            // that case we don't refund anything.
            let refund_imbalance = C::deposit_into_existing(who, refund_amount)
                .unwrap_or_else(|_| C::PositiveImbalance::zero());
            // merge the imbalance caused by paying the fees and refunding parts of it again.
            let adjusted_paid = paid
                .offset(refund_imbalance)
                .same()
                .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
            // Call someone else to handle the imbalance (fee and tip separately)
            let (tip, fee) = adjusted_paid.split(tip);
            OU::on_unbalanceds(Some(fee).into_iter().chain(Some(tip)));
        }
        Ok(())
    }
}

/// Require the transactor pay for themselves and maybe include a tip to gain additional priority
/// in the queue.
///
/// # Transaction Validity
///
/// This extension sets the `priority` field of `TransactionValidity` depending on the amount
/// of tip being paid per weight unit.
///
/// Operational transactions will receive an additional priority bump, so that they are normally
/// considered before regular transactions.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ChargeTransactionPayment<T: Config, D: TokenTransferDetails<T>>(
    #[codec(compact)] BalanceOf<T>,
);

pub trait TokenTransferDetails<T: Config, AssetId> {
    fn amount(&self) -> BalanceOf<T>;
    fn destination_account(&self) -> T::AccountId;
    fn asset_id(&self) -> AssetId;
}

pub trait TokenTransferModule {
    fn asset_transfer_module() -> pallet_ismp::ModuleId;
}

impl<T: Config> ChargeTransactionPayment<T>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    BalanceOf<T>: Send + Sync + FixedPointOperand,
{
    /// utility constructor. Used only in client/factory code.
    pub fn from(fee: BalanceOf<T>) -> Self {
        Self(fee)
    }

    /// Returns the tip as being chosen by the transaction sender.
    pub fn tip(&self) -> BalanceOf<T> {
        self.0
    }

    fn withdraw_fee_from_ismp_message() -> Result<
        (
            BalanceOf<T>,
            <<T as Config>::OnChargeTransaction as OnChargeTransaction<T>>::LiquidityInfo,
        ),
        TransactionValidityError,
    > {
    }

    fn withdraw_fee(
        &self,
        who: &T::AccountId,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
    ) -> Result<
        (
            BalanceOf<T>,
            <<T as Config>::OnChargeTransaction as OnChargeTransaction<T>>::LiquidityInfo,
        ),
        TransactionValidityError,
    > {
        let tip = self.0;
        let fee = Pallet::<T>::compute_fee(len as u32, info, tip);

        <<T as Config>::OnChargeTransaction as OnChargeTransaction<T>>::withdraw_fee(
            who, call, info, fee, tip,
        )
        .map(|i| (fee, i))
    }

    /// Get an appropriate priority for a transaction with the given `DispatchInfo`, encoded length
    /// and user-included tip.
    ///
    /// The priority is based on the amount of `tip` the user is willing to pay per unit of either
    /// `weight` or `length`, depending which one is more limiting. For `Operational` extrinsics
    /// we add a "virtual tip" to the calculations.
    ///
    /// The formula should simply be `tip / bounded_{weight|length}`, but since we are using
    /// integer division, we have no guarantees it's going to give results in any reasonable
    /// range (might simply end up being zero). Hence we use a scaling factor:
    /// `tip * (max_block_{weight|length} / bounded_{weight|length})`, since given current
    /// state of-the-art blockchains, number of per-block transactions is expected to be in a
    /// range reasonable enough to not saturate the `Balance` type while multiplying by the tip.
    pub fn get_priority(
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
        tip: BalanceOf<T>,
        final_fee: BalanceOf<T>,
    ) -> TransactionPriority {
        // Calculate how many such extrinsics we could fit into an empty block and take
        // the limitting factor.
        let max_block_weight = T::BlockWeights::get().max_block;
        let max_block_length = *T::BlockLength::get().max.get(info.class) as u64;

        // TODO: Take into account all dimensions of weight
        let max_block_weight = max_block_weight.ref_time();
        let info_weight = info.weight.ref_time();

        let bounded_weight = info_weight.clamp(1, max_block_weight);
        let bounded_length = (len as u64).clamp(1, max_block_length);

        let max_tx_per_block_weight = max_block_weight / bounded_weight;
        let max_tx_per_block_length = max_block_length / bounded_length;
        // Given our current knowledge this value is going to be in a reasonable range - i.e.
        // less than 10^9 (2^30), so multiplying by the `tip` value is unlikely to overflow the
        // balance type. We still use saturating ops obviously, but the point is to end up with some
        // `priority` distribution instead of having all transactions saturate the priority.
        let max_tx_per_block =
            max_tx_per_block_length.min(max_tx_per_block_weight).saturated_into::<BalanceOf<T>>();
        let max_reward = |val: BalanceOf<T>| val.saturating_mul(max_tx_per_block);

        // To distribute no-tip transactions a little bit, we increase the tip value by one.
        // This means that given two transactions without a tip, smaller one will be preferred.
        let tip = tip.saturating_add(One::one());
        let scaled_tip = max_reward(tip);

        match info.class {
            DispatchClass::Normal => {
                // For normal class we simply take the `tip_per_weight`.
                scaled_tip
            }
            DispatchClass::Mandatory => {
                // Mandatory extrinsics should be prohibited (e.g. by the [`CheckWeight`]
                // extensions), but just to be safe let's return the same priority as `Normal` here.
                scaled_tip
            }
            DispatchClass::Operational => {
                // A "virtual tip" value added to an `Operational` extrinsic.
                // This value should be kept high enough to allow `Operational` extrinsics
                // to get in even during congestion period, but at the same time low
                // enough to prevent a possible spam attack by sending invalid operational
                // extrinsics which push away regular transactions from the pool.
                let fee_multiplier = T::OperationalFeeMultiplier::get().saturated_into();
                let virtual_tip = final_fee.saturating_mul(fee_multiplier);
                let scaled_virtual_tip = max_reward(virtual_tip);

                scaled_tip.saturating_add(scaled_virtual_tip)
            }
        }
        .saturated_into::<TransactionPriority>()
    }
}

impl<T: Config> SignedExtension for ChargeTransactionPayment<T>
where
    BalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = "ChargeTransactionPayment";
    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = (
        // tip
        BalanceOf<T>,
        // who paid the fee - this is an option to allow for a Default impl.
        Self::AccountId,
        // imbalance resulting from withdrawing the fee
        <<T as Config>::OnChargeTransaction as OnChargeTransaction<T>>::LiquidityInfo,
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
        if let Ok((final_fee, _)) = self.withdraw_fee(who, call, info, len) {
            let tip = self.0;

            Ok(ValidTransaction {
                priority: Self::get_priority(info, len, tip, final_fee),
                ..Default::default()
            })
        } else {
            match call.is_sub_type() {
                Some(pallet_ismp::Call::handle { messages }) => {
                    let post = messages.iter().find(|msg| match msg {
                        pallet_ismp::Message::Request(req) => req.requests.iter().find(|post| {
                            <pallet_ismp::ModuleId as Decode>::decode(&mut &*req.to);
                            match id {
                                Ok(id) => id == <M as TokenTransferModule>::asset_transfer_module(),
                                _ => false,
                            }
                        }),
                        _ => false,
                    });
                    match post {
                        Some(post) => {
                            let data = <D as Decode>::decode(&mut &post.data[..])
                                .map_err(|_| TransactionValidityError::Invalid(Payment));
                        }
                        _ => return Err(TransactionValidityError::Invalid(Payment)),
                    }
                }
                _ => Err(TransactionValidityError::Invalid(Payment)),
            }
            Ok(ValidTransaction {
                priority: Self::get_priority(info, len, tip, final_fee),
                ..Default::default()
            })
        }
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        if let Ok((_fee, imbalance)) = self.withdraw_fee(who, call, info, len) {
            Ok((self.0, who.clone(), imbalance))
        } else {
            match call.is_sub_type() {
                Some(pallet_ismp::Call::handle { messages }) => {
                    let post = messages.iter().find(|msg| match msg {
                        pallet_ismp::Message::Request(req) => req.requests.iter().find(|post| {
                            <pallet_ismp::ModuleId as Decode>::decode(&mut &*req.to);
                            match id {
                                Ok(id) => id == <M as TokenTransferModule>::asset_transfer_module(),
                                _ => false,
                            }
                        }),
                        _ => false,
                    });
                    match post {
                        Some(post) => {
                            let data = <D as Decode>::decode(&mut &post.data[..])
                                .map_err(|_| TransactionValidityError::Invalid(Payment));
                        }
                        _ => return Err(TransactionValidityError::Invalid(Payment)),
                    }
                }
                _ => Err(TransactionValidityError::Invalid(Payment)),
            }
            Ok((self.0, who.clone(), imbalance))
        }
    }

    fn post_dispatch(
        maybe_pre: Option<Self::Pre>,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &PostDispatchInfoOf<Self::Call>,
        len: usize,
        _result: &DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        if let Some((tip, who, imbalance)) = maybe_pre {
            let actual_fee = Pallet::<T>::compute_actual_fee(len as u32, info, post_info, tip);
            T::OnChargeTransaction::correct_and_deposit_fee(
                &who, info, post_info, actual_fee, tip, imbalance,
            )?;
            Pallet::<T>::deposit_event(Event::<T>::TransactionFeePaid { who, actual_fee, tip });
        }
        Ok(())
    }
}
