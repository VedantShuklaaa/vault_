use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::state::{VaultState, VAULT_SEED};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = owner,
        space = 8 + VaultState::INIT_SPACE,
        seeds = [VAULT_SEED, owner.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub state: Account<'info, VaultState>,

    #[account(
        init,
        payer = owner,
        associated_token::mint = mint,
        associated_token::authority = state,
        associated_token::token_program = token_program,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    let state = &mut ctx.accounts.state;
    state.owner = ctx.accounts.owner.key();
    state.mint = ctx.accounts.mint.key();
    state.bump = ctx.bumps.state;
    Ok(())
}
