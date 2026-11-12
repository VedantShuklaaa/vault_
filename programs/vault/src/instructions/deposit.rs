use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

use crate::state::{VaultState, VAULT_SEED};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        has_one = owner,
        has_one = mint,
        seeds = [VAULT_SEED, owner.key().as_ref(), mint.key().as_ref()],
        bump = state.bump,
    )]
    pub state: Account<'info, VaultState>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub owner_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = state,
        associated_token::token_program = token_program,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.owner_token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.vault.to_account_info(),
        authority: ctx.accounts.owner.to_account_info(),
    };
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.key(), cpi_accounts);

    transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;
    Ok(())
}
