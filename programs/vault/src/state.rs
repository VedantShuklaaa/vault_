use anchor_lang::prelude::*;

pub const VAULT_SEED: &[u8] = b"vault";

#[account]
#[derive(InitSpace)]
pub struct VaultState {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub bump: u8,
}
