pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use instructions::*;
pub use state::*;

declare_id!("GLihHCi1UMxJqEZd1mTRFfCvhWmAXQfrsZU8awGYKiyC");

#[program]
pub mod vault {
    use super::*;

    /// Creates the vault state PDA and its token account.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        crate::instructions::initialize::handler(ctx)
    }

    /// Moves `amount` tokens from the owner into the vault.
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        crate::instructions::deposit::handler(ctx, amount)
    }

    /// Moves `amount` tokens from the vault back to the owner.
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        crate::instructions::withdraw::handler(ctx, amount)
    }

    /// Withdraws the entire balance, then closes the vault token account and the state.
    pub fn close(ctx: Context<Close>) -> Result<()> {
        crate::instructions::close::handler(ctx)
    }
}
