use crate::primitives::*;
use crate::{BeaconState, Config, Error, utils, consts};
use core::cmp::max;

impl<C: Config> BeaconState<C> {
	pub fn increase_balance(&mut self, index: ValidatorIndex, delta: Gwei) {
		self.balances[index as usize] += delta;
	}

	pub fn decrease_balance(&mut self, index: ValidatorIndex, delta: Gwei) {
		self.balances[index as usize] =
			self.balances[index as usize].saturating_sub(delta);
	}

	pub fn initiate_validator_exit(&mut self, index: ValidatorIndex) {
		if self.validators[index as usize].exit_epoch !=
			consts::FAR_FUTURE_EPOCH
		{
			return
		}

		let exit_epochs = self.validators.iter()
			.map(|v| v.exit_epoch)
			.filter(|epoch| *epoch != consts::FAR_FUTURE_EPOCH)
			.collect::<Vec<_>>();
		let mut exit_queue_epoch = max(
			exit_epochs.iter().fold(0, |a, b| max(a, *b)),
			utils::activation_exit_epoch::<C>(self.current_epoch())
		);
		let exit_queue_churn = self.validators.iter()
			.filter(|v| v.exit_epoch == exit_queue_epoch)
			.count() as u64;

		if exit_queue_churn >= self.validator_churn_limit() {
			exit_queue_epoch += 1;
		}

		let validator = &mut self.validators[index as usize];
		validator.exit_epoch = exit_queue_epoch;
		validator.withdrawable_epoch = validator.exit_epoch +
			C::min_validator_withdrawability_delay();
	}

	pub fn slash_validator(
		&mut self,
		slashed_index: ValidatorIndex,
		whistleblower_index: Option<ValidatorIndex>
	) -> Result<(), Error> {
		let current_epoch = self.current_epoch();
		self.initiate_validator_exit(slashed_index);

		self.validators[slashed_index as usize].slashed = true;
		self.validators[slashed_index as usize].withdrawable_epoch = max(
			self.validators[slashed_index as usize].withdrawable_epoch,
			current_epoch + C::epochs_per_slashings_vector()
		);
		let slashed_balance =
			self.validators[slashed_index as usize].effective_balance;
		self.slashings[
			(current_epoch % C::epochs_per_slashings_vector()) as usize
		] += slashed_balance;
		self.decrease_balance(slashed_index, slashed_balance / C::min_slashing_penalty_quotient());

		let proposer_index = self.beacon_proposer_index()?;
		let whistleblower_index = whistleblower_index.unwrap_or(proposer_index);
		let whistleblowing_reward =
			slashed_balance / C::whistleblower_reward_quotient();
		let proposer_reward =
			whistleblowing_reward / C::proposer_reward_quotient();

		self.increase_balance(proposer_index, proposer_reward);
		self.increase_balance(
			whistleblower_index, whistleblowing_reward - proposer_reward
		);

		Ok(())
	}
}
