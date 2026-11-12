use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
    TransferChecked,
};

use crate::state::{VaultState, VAULT_SEED};

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        close = owner,
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

pub fn handler(ctx: Context<Close>) -> Result<()> {
    let balance = ctx.accounts.vault.amount;
    let seeds: &[&[u8]] = &[
        VAULT_SEED,
        ctx.accounts.state.owner.as_ref(),
        ctx.accounts.state.mint.as_ref(),
        &[ctx.accounts.state.bump],
    ];
    let signer_seeds: &[&[&[u8]]] = &[seeds];

    if balance > 0 {
        let cpi_accounts = TransferChecked {
            from: ctx.accounts.vault.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.owner_token_account.to_account_info(),
            authority: ctx.accounts.state.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            cpi_accounts,
            signer_seeds,
        );

        transfer_checked(cpi_ctx, balance, ctx.accounts.mint.decimals)?;
    }

    let close_accounts = CloseAccount {
        account: ctx.accounts.vault.to_account_info(),
        destination: ctx.accounts.owner.to_account_info(),
        authority: ctx.accounts.state.to_account_info(),
    };
    let close_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        close_accounts,
        signer_seeds,
    );

    close_account(close_ctx)?;
    Ok(())
}
