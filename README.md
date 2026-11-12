# Vault

A minimal SPL token vault built with Anchor. An owner deposits tokens of one mint into a vault, withdraws them later, and can close the vault to get everything back, including rent. Works with both Token and Token-2022 (`transfer_checked` everywhere).

## State

`VaultState` is a PDA with seeds `["vault", owner, mint]`.

| Field   | Type     | Purpose                          |
| ------- | -------- | -------------------------------- |
| `owner` | `Pubkey` | Only account allowed to use the vault |
| `mint`  | `Pubkey` | Token mint the vault holds       |
| `bump`  | `u8`     | Bump used to sign as the PDA     |

The vault token account is the ATA of the state PDA, so it is derived and not stored.

## Instructions

| Instruction        | Who signs | What happens                                              |
| ------------------ | --------- | --------------------------------------------------------- |
| `initialize()`     | owner     | Creates the state PDA and the vault token account         |
| `deposit(amount)`  | owner     | Moves tokens from the owner's account into the vault      |
| `withdraw(amount)` | owner     | Moves tokens from the vault back to the owner (PDA signs) |
| `close()`          | owner     | Withdraws everything, closes the vault account and state, rent goes to the owner |

`withdraw` fails with `ZeroAmount` if the amount is 0, and with `InsufficientVaultBalance` if it exceeds the vault balance.

## Build

```sh
anchor build
```

Output: `target/deploy/vault.so` and `target/idl/vault.json`.

## Gotchas

- Run `anchor build` from the workspace root. Anchor resolves `target/deploy/` relative to the current directory, so building from `programs/vault/` makes it generate a second keypair there and fail with `Program ID mismatch`.
- One vault per `(owner, mint)`. Close it before creating it again.
- Token-2022 transfer fees mean the vault can receive less than `amount`. `withdraw` is capped by the real token balance.
- Mints with a transfer hook that needs extra accounts are not supported.
- Native SOL is not supported. Use wrapped SOL.

## Tests

```sh
cargo test -p vault
```

Uses LiteSVM with the program loaded from `target/deploy/vault.so`, so run
`anchor build` first. Covers initialize, deposit, withdraw, close,
owner-only access, wrong-mint deposits, zero/over-balance withdrawals, and
re-initialization after close.
