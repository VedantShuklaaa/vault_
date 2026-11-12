use litesvm::{types::TransactionResult, LiteSVM};
use litesvm_token::{CreateAssociatedTokenAccount, CreateMint, MintTo};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_instruction_error::InstructionError;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_transaction_error::TransactionError;
use std::str::FromStr;

const PROGRAM_ID: &str = "2mE7UH67aZXAeveviE2rN2Y5FsysXbjrtJXxFK1WBQLa";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const MINTED: u64 = 1_000;

const ZERO_AMOUNT: u32 = 6000;
const INSUFFICIENT_BALANCE: u32 = 6001;

// ---------- helpers ----------

fn id(s: &str) -> Address {
    Address::from_str(s).unwrap()
}

fn system_program() -> Address {
    Address::default()
}

// Everything a test needs: the SVM, the owner, and the vault addresses.
struct Ctx {
    svm: LiteSVM,
    owner: Keypair,
    mint: Address,
    owner_ata: Address,
    state: Address,
    bump: u8,
    vault: Address,
}

fn ata(owner: &Address, mint: &Address) -> Address {
    let token = id(TOKEN_PROGRAM);
    Address::find_program_address(
        &[owner.as_ref(), token.as_ref(), mint.as_ref()],
        &id(ATA_PROGRAM),
    )
    .0
}

// Fresh SVM with the program loaded, a funded owner, a mint and 1000 tokens in the owner's ATA.
fn setup() -> Ctx {
    let mut svm = LiteSVM::new();
    let program_id = id(PROGRAM_ID);
    svm.add_program_from_file(program_id, "../../target/deploy/vault.so")
        .unwrap();

    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 10_000_000_000).unwrap();

    let mint = CreateMint::new(&mut svm, &owner)
        .decimals(6)
        .send()
        .unwrap();
    let owner_pk = owner.pubkey();
    let owner_ata = CreateAssociatedTokenAccount::new(&mut svm, &owner, &mint)
        .owner(&owner_pk)
        .send()
        .unwrap();
    MintTo::new(&mut svm, &owner, &mint, &owner_ata, MINTED)
        .send()
        .unwrap();

    let (state, bump) = Address::find_program_address(
        &[b"vault", owner.pubkey().as_ref(), mint.as_ref()],
        &program_id,
    );
    let vault = ata(&state, &mint);

    Ctx {
        svm,
        owner,
        mint,
        owner_ata,
        state,
        bump,
        vault,
    }
}

// setup() plus a successful initialize.
fn setup_with_vault() -> Ctx {
    let mut c = setup();
    let ix = ix_initialize(&c);
    send(&mut c.svm, ix, &c.owner).unwrap();
    c
}

// New blockhash each time so identical transactions are not rejected as duplicates.
// The failure type comes from LiteSVM and is large; returning it as-is keeps the
// call sites simple.
#[allow(clippy::result_large_err)]
fn send(svm: &mut LiteSVM, ix: Instruction, signer: &Keypair) -> TransactionResult {
    svm.expire_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
}

// Anchor instruction data = first 8 bytes of sha256("global:<name>") + args.
fn data(name: &str, arg: Option<u64>) -> Vec<u8> {
    let mut d = Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec();
    if let Some(a) = arg {
        d.extend_from_slice(&a.to_le_bytes());
    }
    d
}

// ACCOUNT ORDER BELOW MUST MATCH YOUR #[derive(Accounts)] STRUCTS.
fn ix_initialize(c: &Ctx) -> Instruction {
    Instruction {
        program_id: id(PROGRAM_ID),
        accounts: vec![
            AccountMeta::new(c.owner.pubkey(), true),
            AccountMeta::new_readonly(c.mint, false),
            AccountMeta::new(c.state, false),
            AccountMeta::new(c.vault, false),
            AccountMeta::new_readonly(id(TOKEN_PROGRAM), false),
            AccountMeta::new_readonly(id(ATA_PROGRAM), false),
            AccountMeta::new_readonly(system_program(), false),
        ],
        data: data("initialize", None),
    }
}

// signer / mint / signer_ata are parameters so tests can pass wrong ones.
// Order matches the Deposit/Withdraw/Close structs: owner, state, mint,
// owner_token_account, vault, token_program.
fn ix_transfer(
    c: &Ctx,
    name: &str,
    signer: &Address,
    mint: &Address,
    signer_ata: &Address,
    amount: Option<u64>,
) -> Instruction {
    Instruction {
        program_id: id(PROGRAM_ID),
        accounts: vec![
            AccountMeta::new(*signer, true),
            AccountMeta::new(c.state, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*signer_ata, false),
            AccountMeta::new(c.vault, false),
            AccountMeta::new_readonly(id(TOKEN_PROGRAM), false),
        ],
        data: data(name, amount),
    }
}

fn deposit(c: &mut Ctx, amount: u64) {
    let ix = ix_transfer(
        c,
        "deposit",
        &c.owner.pubkey(),
        &c.mint,
        &c.owner_ata,
        Some(amount),
    );
    send(&mut c.svm, ix, &c.owner).unwrap();
}

// SPL token account layout: mint 0..32, owner 32..64, amount 64..72.
fn balance(svm: &LiteSVM, addr: &Address) -> u64 {
    let acc = svm.get_account(addr).unwrap();
    u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
}

fn assert_custom(res: TransactionResult, code: u32) {
    match res.unwrap_err().err {
        TransactionError::InstructionError(_, InstructionError::Custom(c)) => assert_eq!(c, code),
        other => panic!("expected custom error {code}, got {other:?}"),
    }
}

// A funded non-owner with their own token account holding tokens.
fn make_attacker(c: &mut Ctx) -> (Keypair, Address) {
    let attacker = Keypair::new();
    c.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let a_ata = CreateAssociatedTokenAccount::new(&mut c.svm, &attacker, &c.mint)
        .owner(&attacker.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut c.svm, &c.owner, &c.mint, &a_ata, MINTED)
        .send()
        .unwrap();
    (attacker, a_ata)
}

// ---------- initialize ----------

#[test]
fn initialize_creates_state_and_vault() {
    let c = setup_with_vault();

    let state = c.svm.get_account(&c.state).unwrap();
    assert_eq!(state.owner, id(PROGRAM_ID));
    assert_eq!(&state.data[8..40], c.owner.pubkey().as_ref());
    assert_eq!(&state.data[40..72], c.mint.as_ref());
    assert_eq!(state.data[72], c.bump);

    // vault is a token account for the mint, authority = state PDA
    let vault = c.svm.get_account(&c.vault).unwrap();
    assert_eq!(&vault.data[0..32], c.mint.as_ref());
    assert_eq!(&vault.data[32..64], c.state.as_ref());
    assert_eq!(balance(&c.svm, &c.vault), 0);
}

#[test]
fn initialize_twice_fails() {
    let mut c = setup_with_vault();
    let ix = ix_initialize(&c);
    assert!(send(&mut c.svm, ix, &c.owner).is_err());
}

// ---------- deposit ----------

#[test]
fn deposit_moves_tokens_into_vault() {
    let mut c = setup_with_vault();
    assert_eq!(balance(&c.svm, &c.owner_ata), 1_000);
    assert_eq!(balance(&c.svm, &c.vault), 0);

    deposit(&mut c, 400);

    assert_eq!(balance(&c.svm, &c.owner_ata), 600);
    assert_eq!(balance(&c.svm, &c.vault), 400);
}

#[test]
fn deposit_by_non_owner_fails() {
    let mut c = setup_with_vault();
    let (attacker, a_ata) = make_attacker(&mut c);

    let ix = ix_transfer(
        &c,
        "deposit",
        &attacker.pubkey(),
        &c.mint,
        &a_ata,
        Some(100),
    );
    assert!(send(&mut c.svm, ix, &attacker).is_err());

    assert_eq!(balance(&c.svm, &a_ata), 1_000);
    assert_eq!(balance(&c.svm, &c.vault), 0);
}

#[test]
fn deposit_with_wrong_mint_fails() {
    let mut c = setup_with_vault();
    let mint2 = CreateMint::new(&mut c.svm, &c.owner)
        .decimals(6)
        .send()
        .unwrap();
    let ata2 = CreateAssociatedTokenAccount::new(&mut c.svm, &c.owner, &mint2)
        .owner(&c.owner.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut c.svm, &c.owner, &mint2, &ata2, 500)
        .send()
        .unwrap();

    let ix = ix_transfer(&c, "deposit", &c.owner.pubkey(), &mint2, &ata2, Some(100));
    assert!(send(&mut c.svm, ix, &c.owner).is_err());

    assert_eq!(balance(&c.svm, &ata2), 500);
    assert_eq!(balance(&c.svm, &c.vault), 0);
}

// ---------- withdraw ----------

#[test]
fn withdraw_partial() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);
    assert_eq!(balance(&c.svm, &c.owner_ata), 500);
    assert_eq!(balance(&c.svm, &c.vault), 500);

    let ix = ix_transfer(
        &c,
        "withdraw",
        &c.owner.pubkey(),
        &c.mint,
        &c.owner_ata,
        Some(200),
    );
    send(&mut c.svm, ix, &c.owner).unwrap();

    assert_eq!(balance(&c.svm, &c.owner_ata), 700);
    assert_eq!(balance(&c.svm, &c.vault), 300);
}

#[test]
fn withdraw_full_leaves_vault_empty() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);

    let ix = ix_transfer(
        &c,
        "withdraw",
        &c.owner.pubkey(),
        &c.mint,
        &c.owner_ata,
        Some(500),
    );
    send(&mut c.svm, ix, &c.owner).unwrap();

    assert_eq!(balance(&c.svm, &c.owner_ata), 1_000);
    assert_eq!(balance(&c.svm, &c.vault), 0);
}

#[test]
fn withdraw_zero_fails() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);

    let ix = ix_transfer(
        &c,
        "withdraw",
        &c.owner.pubkey(),
        &c.mint,
        &c.owner_ata,
        Some(0),
    );
    assert_custom(send(&mut c.svm, ix, &c.owner), ZERO_AMOUNT);

    assert_eq!(balance(&c.svm, &c.vault), 500);
}

#[test]
fn withdraw_more_than_balance_fails() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);

    let ix = ix_transfer(
        &c,
        "withdraw",
        &c.owner.pubkey(),
        &c.mint,
        &c.owner_ata,
        Some(501),
    );
    assert_custom(send(&mut c.svm, ix, &c.owner), INSUFFICIENT_BALANCE);

    assert_eq!(balance(&c.svm, &c.vault), 500);
}

#[test]
fn withdraw_by_non_owner_fails() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);
    let (attacker, a_ata) = make_attacker(&mut c);

    let ix = ix_transfer(
        &c,
        "withdraw",
        &attacker.pubkey(),
        &c.mint,
        &a_ata,
        Some(100),
    );
    assert!(send(&mut c.svm, ix, &attacker).is_err());

    assert_eq!(balance(&c.svm, &a_ata), 1_000);
    assert_eq!(balance(&c.svm, &c.vault), 500);
}

// ---------- close ----------

// close has no args; same account order as withdraw is assumed.
fn ix_close(c: &Ctx, signer: &Address, signer_ata: &Address) -> Instruction {
    ix_transfer(c, "close", signer, &c.mint, signer_ata, None)
}

#[test]
fn close_with_balance_returns_everything() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);
    assert_eq!(balance(&c.svm, &c.owner_ata), 500);
    let lamports_before = c.svm.get_balance(&c.owner.pubkey()).unwrap();

    let ix = ix_close(&c, &c.owner.pubkey(), &c.owner_ata);
    send(&mut c.svm, ix, &c.owner).unwrap();

    assert_eq!(balance(&c.svm, &c.owner_ata), 1_000);
    assert!(c.svm.get_account(&c.vault).is_none());
    assert!(c.svm.get_account(&c.state).is_none());
    // rent from both closed accounts comes back (far more than the tx fee)
    assert!(c.svm.get_balance(&c.owner.pubkey()).unwrap() > lamports_before);
}

#[test]
fn close_with_zero_balance_works() {
    let mut c = setup_with_vault();

    let ix = ix_close(&c, &c.owner.pubkey(), &c.owner_ata);
    send(&mut c.svm, ix, &c.owner).unwrap();

    assert_eq!(balance(&c.svm, &c.owner_ata), 1_000);
    assert!(c.svm.get_account(&c.vault).is_none());
    assert!(c.svm.get_account(&c.state).is_none());
}

#[test]
fn close_by_non_owner_fails() {
    let mut c = setup_with_vault();
    deposit(&mut c, 500);
    let (attacker, a_ata) = make_attacker(&mut c);

    let ix = ix_close(&c, &attacker.pubkey(), &a_ata);
    assert!(send(&mut c.svm, ix, &attacker).is_err());

    assert_eq!(balance(&c.svm, &c.vault), 500);
    assert!(c.svm.get_account(&c.state).is_some());
}

#[test]
fn reinitialize_after_close_works() {
    let mut c = setup_with_vault();

    let ix = ix_close(&c, &c.owner.pubkey(), &c.owner_ata);
    send(&mut c.svm, ix, &c.owner).unwrap();
    assert!(c.svm.get_account(&c.state).is_none());

    let ix = ix_initialize(&c);
    send(&mut c.svm, ix, &c.owner).unwrap();

    assert!(c.svm.get_account(&c.state).is_some());
    assert_eq!(balance(&c.svm, &c.vault), 0);
}
