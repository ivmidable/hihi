use crate::{
    error::HihiError,
    instruction::{Breach, Claim, HihiInstruction, Initialize},
    state::HihiState,
};

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    hash::hash,
    msg,
    native_token::{lamports_to_sol, sol_to_lamports},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};

const LB_DIFF: u8 = 3;
const MAX_DIFF: u8 = 23;
const START_DIFF: u8 = 2;
const BREACH_WINDOW: u16 = 100;
const START_PRICE: u64 = 150000000;
const LB_TOKEN_COUNT: u8 = 200;
const LB_DIFF_INCREASE: u8 = 5;
const LB_MAX_PER_EPOCH: u8 = 23;

pub struct Processor {}
impl Processor {
    pub fn authority_id(
        program_id: &Pubkey,
        my_info: &Pubkey,
        nonce: u8,
    ) -> Result<Pubkey, HihiError> {
        Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
            .or(Err(HihiError::InvalidProgramAddress))
    }

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
        instance: &Pubkey,
        token_program: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let instance_bytes = instance.to_bytes();
        let authority_signature_seeds = [&instance_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            &spl_token::id(),
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(&ix, &[mint, destination, authority, token_program], signers)
    }

    pub fn process_initialize(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        nonce: &u8,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let instance_info = next_account_info(account_info_iter)?;
        let initializer_info = next_account_info(account_info_iter)?;
        let token_mint_info = next_account_info(account_info_iter)?;
        let admin_one_info = next_account_info(account_info_iter)?;
        let admin_two_info = next_account_info(account_info_iter)?;
        let withdraw_info = next_account_info(account_info_iter)?;

        let initializer_id = Pubkey::new(&[
            80, 97, 223, 1, 83, 109, 8, 147, 151, 40, 159, 3, 204, 231, 107, 20, 85, 34, 21, 236,
            209, 141, 60, 7, 160, 185, 70, 160, 206, 226, 231, 158,
        ]);

        if instance_info.owner != program_id
            || instance_info.is_signer == false
            || initializer_info.is_signer == false
            || initializer_info.key != &initializer_id
            || admin_one_info.is_signer == false
            || admin_two_info.is_signer == false
            || withdraw_info.is_signer == false
        {
            return Err(HihiError::InvalidOwner.into());
        }

        let rent_info = next_account_info(account_info_iter)?;

        let instance_data_len = instance_info.data_len();
        let mut instance = HihiState::unpack_unchecked(&instance_info.data.borrow_mut())?;

        if instance.is_initialized {
            return Err(HihiError::AlreadyInitialized.into());
        }

        let rent = Rent::from_account_info(rent_info)?;

        if !rent.is_exempt(instance_info.lamports(), instance_data_len) {
            return Err(HihiError::NotRentExempt.into());
        }
        instance.admin_one_id = *admin_one_info.key;
        instance.admin_two_id = *admin_two_info.key;
        instance.withdraw_id = *withdraw_info.key;

        instance.token_mint_id = *token_mint_info.key;

        let clock = Clock::get()?;

        instance.current_slot = clock.slot;
        instance.current_epoch = clock.epoch;
        instance.breach_price = calculate_price(instance.breach_count, START_PRICE);

        instance.difficulty = START_DIFF;

        instance.limit_break = create_limit_break(
            &clock,
            &instance,
            instance_info.key,
            LB_TOKEN_COUNT,
            instance.difficulty + LB_DIFF,
        );

        instance.nonce = *nonce;

        instance.is_initialized = true;
        HihiState::pack(instance, &mut instance_info.data.borrow_mut())?;
        Ok(())
    }

    pub fn process_breach(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        lamports: u64,
    ) -> ProgramResult {
        if lamports < 10000 {
            return Err(HihiError::InsufficientFundsForTransaction.into());
        }

        let account_info_iter = &mut accounts.iter();
        let instance_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let token_mint_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let from_info = next_account_info(account_info_iter)?;
        let to_token_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;

        if instance_info.owner != program_id || instance_info.is_writable == false {
            return Err(HihiError::InvalidOwner.into());
        }

        if from_info.is_signer == false || from_info.is_writable == false {
            return Err(HihiError::InvalidInstruction.into());
        }

        let mut instance = HihiState::unpack_unchecked(&instance_info.data.borrow_mut())?;

        if instance.is_initialized == false {
            return Err(HihiError::NotInitialized.into());
        }

        let id = Self::authority_id(program_id, instance_info.key, instance.nonce)?;

        if &id != authority_info.key {
            return Err(HihiError::InvalidOwner.into());
        }

        let mut breaches: u64 = 0;
        let mut b_tokens = instance.breach_count
            - instance.breach_count_this_window as i32
            - instance.work_cached as i32;
        if b_tokens < 0 {
            b_tokens = 0;
        }
        let base_tokens = calculate_tokens(b_tokens);

        let clock = Clock::get()?;

        if instance.difficulty + LB_DIFF <= MAX_DIFF {
            //Transfer Lamports.
            let ix = solana_program::system_instruction::transfer(
                from_info.key,
                authority_info.key,
                lamports,
            );
            invoke(
                &ix,
                &[
                    from_info.clone(),
                    authority_info.clone(),
                    system_program_info.clone(),
                ],
            )?;

            instance.lamports = instance.lamports + (lamports / 4) * 3;

            let valid_to_id = check_accounts(
                &instance,
                token_program_info.key,
                token_mint_info.key,
                to_token_info,
            )?;

            if clock.slot - instance.current_slot >= BREACH_WINDOW as u64 {
                instance.breach_count_this_window = 0;
                instance.current_slot = clock.slot;
                instance.breach_price = calculate_price(instance.breach_count, START_PRICE);
            }

            if lamports > instance.breach_price * 10 {
                return Err(HihiError::InsufficientFundsForTransaction.into());
            }

            let sol = lamports_to_sol(lamports);
            let bp_sol = lamports_to_sol(instance.breach_price);
            let br_sol = lamports_to_sol(instance.breach_remain);
            let result: f64 = (sol + br_sol) / bp_sol;

            let sf = split_float(result);
            breaches = sf.0;
            instance.breach_remain = sol_to_lamports(bp_sol * sf.1);

            if valid_to_id == true {
                let tokens_to_send = base_tokens as u64 * breaches;

                Self::token_mint_to(
                    instance_info.key,
                    token_program_info.clone(),
                    token_mint_info.clone(),
                    to_token_info.clone(),
                    authority_info.clone(),
                    instance.nonce,
                    sol_to_lamports(tokens_to_send as f64),
                )?;
            } else {
                instance.token_doubles += breaches;
            }

            //change hash of limit break.
            if breaches > 0 {
                instance.limit_break = create_limit_break(
                    &clock,
                    &instance,
                    instance_info.key,
                    LB_TOKEN_COUNT,
                    instance.difficulty + LB_DIFF,
                );
            }
        }

        let free = instance.get_work_free_space();

        if free != 0 {
            let mut count = 0;
            let total = breaches + instance.work_cached;
            if total <= 10 {
                instance.work_cached = 0;
                count = total;
            } else {
                instance.work_cached -= 10 - breaches;
                count = 10;
            }

            let remain = count as i32 - free;

            if remain > 0 {
                instance.work_cached += remain as u64;
                count = count - remain as u64;
            }

            let work = create_hash_puzzles(
                &clock,
                count as u8,
                &instance,
                instance_info.key,
                lamports,
                base_tokens,
                instance.difficulty,
            );

            instance.add_work(work.0.as_slice())?;
            instance.token_doubles = work.1;
        } else {
            instance.work_cached += breaches;
        }

        if instance.breach_count + breaches as i32 > i32::MAX {
            return Err(HihiError::InvalidInstruction.into());
        }

        instance.breach_count += breaches as i32;
        instance.breach_count_this_window += breaches as u32;

        HihiState::pack(instance, &mut instance_info.data.borrow_mut())?;
        Ok(())
    }

    pub fn process_limit_break(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        Self::process_claim_and_breaks(program_id, accounts, None)?;
        Ok(())
    }

    pub fn process_claim(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        work: [u8; 57],
    ) -> ProgramResult {
        Self::process_claim_and_breaks(program_id, accounts, Some(work))?;
        Ok(())
    }

    fn process_claim_and_breaks(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        work: Option<[u8; 57]>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let instance_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;
        let token_mint_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let claim_info = next_account_info(account_info_iter)?;
        let pool_info = next_account_info(account_info_iter)?;
        let to_token_info = next_account_info(account_info_iter)?;

        if instance_info.owner != program_id
            || instance_info.is_writable == false
            || claim_info.is_signer == false
            || pool_info.is_signer == false
        {
            return Err(HihiError::InvalidOwner.into());
        }

        let mut instance = HihiState::unpack_unchecked(&instance_info.data.borrow_mut())?;

        if instance.is_initialized == false {
            return Err(HihiError::NotInitialized.into());
        }

        let valid_to_id = check_accounts(
            &instance,
            token_program_info.key,
            token_mint_info.key,
            to_token_info,
        )?;

        if valid_to_id == false {
            return Err(HihiError::InvalidTokenAddress.into());
        }

        if let Some(work) = work {
            let work_vec = work.to_vec();

            let mut index = -1;

            for (i, wrk) in instance.work.iter().enumerate() {
                if wrk.iter().eq(work_vec.iter()) {
                    index = i as i32;
                    break;
                }
            }

            if index == -1 {
                return Err(HihiError::InvalidClaimHash.into());
            }
            check_claim(claim_info.key, pool_info.key, &work)?;

            Self::token_mint_to(
                instance_info.key,
                token_program_info.clone(),
                token_mint_info.clone(),
                to_token_info.clone(),
                authority_info.clone(),
                instance.nonce,
                sol_to_lamports(work[0] as f64),
            )?;

            //remove work from heap.
            instance.remove_work(index as usize)?;
        } else {
            //limit break
            let to_lamports_info = next_account_info(account_info_iter)?;
            let system_program_info = next_account_info(account_info_iter)?;

            if instance.difficulty + LB_DIFF > MAX_DIFF {
                return Err(HihiError::InvalidInstruction.into());
            }

            check_claim(
                claim_info.key,
                pool_info.key,
                instance.limit_break.as_slice(),
            )?;

            let count: u8 = LB_TOKEN_COUNT / calculate_tokens(instance.breach_count);
            if instance.breach_count + count as i32 > i32::MAX {
                return Err(HihiError::InvalidInstruction.into());
            }

            Self::token_mint_to(
                instance_info.key,
                token_program_info.clone(),
                token_mint_info.clone(),
                to_token_info.clone(),
                authority_info.clone(),
                instance.nonce,
                sol_to_lamports(instance.limit_break[0] as f64),
            )?;
            let clock = Clock::get()?;

            if clock.slot - instance.current_slot >= BREACH_WINDOW as u64 {
                instance.breach_count_this_window = 0;
                instance.current_slot = clock.slot;
                instance.breach_price = calculate_price(instance.breach_count, START_PRICE);
            }

            //for testing use slots for epochs instead of epochs
            //if clock.slot - instance.current_epoch >= 200 {
            if clock.epoch - instance.current_epoch > 0 {
                if instance.limit_breaks_this_epoch > LB_DIFF_INCREASE as u32 {
                    instance.difficulty += 1;
                    if instance.difficulty + LB_DIFF > MAX_DIFF {
                        let account = authority_info.lamports();
                        send_lamports(
                            account,
                            instance_info.key,
                            instance.nonce,
                            authority_info,
                            to_lamports_info,
                            system_program_info,
                        )?;
                        instance.lamports = 0;
                    } else {
                        let amount = instance.lamports * 5 / 100;
                        send_lamports(
                            amount,
                            instance_info.key,
                            instance.nonce,
                            authority_info,
                            to_lamports_info,
                            system_program_info,
                        )?;
                        instance.lamports = instance.lamports - amount;
                    }
                }
                instance.limit_breaks_this_epoch = 0;
                instance.current_epoch = clock.epoch;
            }

            if instance.limit_breaks_this_epoch > LB_MAX_PER_EPOCH as u32 {
                return Err(HihiError::WorkLimitExceeded.into());
            }

            instance.breach_count += count as i32;
            instance.breach_count_this_window += count as u32;

            instance.limit_breaks_this_epoch += 1;

            instance.limit_count += 1;

            if instance.difficulty + LB_DIFF <= MAX_DIFF {
                instance.limit_break = create_limit_break(
                    &clock,
                    &instance,
                    instance_info.key,
                    LB_TOKEN_COUNT,
                    instance.difficulty + LB_DIFF,
                );
            }
        }

        HihiState::pack(instance, &mut instance_info.data.borrow_mut())?;
        Ok(())
    }

    pub fn process_withdraw(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let instance_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let withdraw_info = next_account_info(account_info_iter)?;
        let system_program_info = next_account_info(account_info_iter)?;

        let instance = HihiState::unpack_unchecked(&instance_info.data.borrow())?;

        if instance_info.owner != program_id
            || withdraw_info.key != &instance.withdraw_id
            || withdraw_info.is_signer == false
        {
            return Err(HihiError::InvalidOwner.into());
        }

        let account = authority_info.lamports();
        if account <= instance.lamports {
            return Err(HihiError::InsufficientFundsForTransaction.into());
        }
        let amount = account - instance.lamports;

        send_lamports(
            amount,
            instance_info.key,
            instance.nonce,
            authority_info,
            withdraw_info,
            system_program_info,
        )?;

        HihiState::pack(instance, &mut instance_info.data.borrow_mut())?;
        Ok(())
    }

    pub fn process_change_keys(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let instance_info = next_account_info(account_info_iter)?;
        let admin_one_info = next_account_info(account_info_iter)?;
        let admin_two_info = next_account_info(account_info_iter)?;
        let withdraw_info = next_account_info(account_info_iter)?;
        let new_admin_one_info = next_account_info(account_info_iter)?;
        let new_admin_two_info = next_account_info(account_info_iter)?;
        let new_withdraw_info = next_account_info(account_info_iter)?;

        if instance_info.owner != program_id || instance_info.is_writable == false {
            return Err(HihiError::InvalidOwner.into());
        }

        if admin_one_info.is_signer == false
            || admin_two_info.is_signer == false
            || withdraw_info.is_signer == false
            || new_admin_one_info.is_signer == false
            || new_admin_two_info.is_signer == false
            || new_withdraw_info.is_signer == false
        {
            return Err(HihiError::InvalidOwner.into());
        }

        let mut instance = HihiState::unpack_unchecked(&instance_info.data.borrow_mut())?;

        if instance.is_initialized == false {
            return Err(HihiError::NotInitialized.into());
        }

        if admin_one_info.key != &instance.admin_one_id
            || admin_two_info.key != &instance.admin_two_id
            || withdraw_info.key != &instance.withdraw_id
        {
            return Err(HihiError::InvalidOwner.into());
        }

        instance.admin_one_id = *new_admin_one_info.key;
        instance.admin_two_id = *new_admin_two_info.key;
        instance.withdraw_id = *new_withdraw_info.key;

        HihiState::pack(instance, &mut instance_info.data.borrow_mut())?;
        Ok(())
    }

    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = HihiInstruction::unpack(input)?;
        match instruction {
            HihiInstruction::Initialize(Initialize { nonce }) => {
                msg!("Instruction: Initialize");
                return Self::process_initialize(program_id, accounts, &nonce);
            }
            HihiInstruction::Breach(Breach { lamports }) => {
                msg!("Instruction: Breach");
                return Self::process_breach(program_id, accounts, lamports);
            }
            HihiInstruction::LimitBreak => {
                msg!("Instruction: Limit Break");
                return Self::process_limit_break(program_id, accounts);
            }
            HihiInstruction::Claim(Claim { work }) => {
                msg!("Instruction: Claim");
                return Self::process_claim(program_id, accounts, work);
            }
            HihiInstruction::Withdraw => {
                msg!("Instruction: Withdraw");
                return Self::process_withdraw(program_id, accounts);
            }
            HihiInstruction::ChangeKeys => {
                msg!("Instruction: Change Keys");
                return Self::process_change_keys(program_id, accounts);
            }
        }
    }
}

pub fn send_lamports<'a>(
    amount: u64,
    instance_id: &Pubkey,
    nonce: u8,
    authority_info: &AccountInfo<'a>,
    to_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
) -> ProgramResult {
    let instance_bytes = instance_id.to_bytes();
    let authority_signature_seeds = [&instance_bytes[..32], &[nonce]];
    let signers = &[&authority_signature_seeds[..]];

    //Transfer Lamports.
    let ix = solana_program::system_instruction::transfer(authority_info.key, to_info.key, amount);

    invoke_signed(
        &ix,
        &[
            authority_info.clone(),
            to_info.clone(),
            system_program_info.clone(),
        ],
        signers,
    )?;
    Ok(())
}

pub fn create_limit_break(
    clock: &Clock,
    instance: &HihiState,
    instance_id: &Pubkey,
    claimable_tokens: u8,
    magic_len: u8,
) -> Vec<u8> {
    let mut out_vec = Vec::<u8>::new();
    let mut data_vec = instance_id.to_bytes().to_vec();
    data_vec.extend_from_slice(&instance.token_mint_id.to_bytes());
    data_vec.extend_from_slice(&instance.breach_count.to_le_bytes());
    data_vec.extend_from_slice(&instance.breach_price.to_le_bytes());
    data_vec.extend_from_slice(&instance.limit_count.to_le_bytes());
    data_vec.extend_from_slice(&clock.slot.to_le_bytes());
    data_vec.extend_from_slice(&clock.epoch.to_le_bytes());
    data_vec.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
    out_vec.push(claimable_tokens);
    out_vec.extend_from_slice(&hash(data_vec.as_slice()).to_bytes());
    out_vec.extend_from_slice(&[
        magic_len, 33, 232, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    return out_vec;
}

//magic len must be below < 17
pub fn create_hash_puzzles(
    clock: &Clock,
    count: u8,
    instance: &HihiState,
    instance_id: &Pubkey,
    lamports_paid: u64,
    claimable_tokens: u8,
    magic_len: u8,
) -> (Vec<u8>, u64) {
    let mut out_vec = Vec::<u8>::new();
    let mut doubles = instance.token_doubles;
    let mut data_vec = instance_id.to_bytes().to_vec();
    data_vec.extend_from_slice(&instance.token_mint_id.to_bytes());
    data_vec.extend_from_slice(&lamports_paid.to_le_bytes());
    data_vec.extend_from_slice(&instance.breach_count.to_le_bytes());
    data_vec.extend_from_slice(&instance.breach_price.to_le_bytes());
    data_vec.extend_from_slice(&clock.slot.to_le_bytes());
    data_vec.extend_from_slice(&clock.epoch.to_le_bytes());
    data_vec.extend_from_slice(&clock.unix_timestamp.to_le_bytes());
    let mut hash_vec = hash(data_vec.as_slice()).to_bytes().to_vec();
    for i in (0..count).rev() {
        hash_vec.push(i);
        if doubles > 0 {
            out_vec.push(claimable_tokens * 2);
            doubles = doubles - 1;
        } else {
            out_vec.push(claimable_tokens);
        }
        out_vec.extend_from_slice(&hash(hash_vec.as_slice()).to_bytes());
        out_vec.extend_from_slice(&[
            magic_len, 33, 232, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
    }
    return (out_vec, doubles);
}

pub fn calculate_tokens(count: i32) -> u8 {
    if count < 1000 {
        return 100;
    } else if count >= 1000 && count < 10000 {
        return 50;
    } else if count >= 10000 && count < 100000 {
        return 25;
    } else {
        return 10;
    }
}

//add some precompute to higher counts to save on-chain compute.
pub fn calculate_price(count: i32, start_price: u64) -> u64 {
    let mut price: u64 = 0;
    if count < 1000 {
        price = price_grower(count, start_price as f64, 0.00218);
    } else if count >= 1000 && count < 10000 {
        price = 1323796464; //precompute
        price = price_grower(count - 1000, price as f64, 0.000218);
    } else if count >= 10000 && count < 100000 {
        price = 9416424207; //precompute.
        price = price_grower(count - 10000, price as f64, 0.0000218);
    } else {
        price = 67051171537; //precompute.
        price = price_grower(count - 100000, price as f64, 0.00000218);
    }
    return price;
}

pub fn price_grower(count: i32, price: f64, rate: f32) -> u64 {
    let p: f64 = price * f32::powi(1.0 + rate / 1.0, count) as f64;
    return ceil(p);
}

pub fn check_claim(claim_id: &Pubkey, pool_id: &Pubkey, work: &[u8]) -> ProgramResult {
    let (_tokens, rest) = work.split_at(1);
    let (sha, rest) = rest.split_at(32);
    let (mag_len, rest) = rest.split_at(1);
    let (magic, _rest) = rest.split_at(mag_len[0] as usize);
    let mut data_vec = sha.to_vec();
    data_vec.extend_from_slice(&claim_id.to_bytes());
    data_vec.extend_from_slice(&pool_id.to_bytes());
    let hash_vec = hash(data_vec.as_slice()).to_bytes().to_vec();
    if hash_vec.starts_with(magic) == false {
        return Err(HihiError::IncorrectClaimSolution.into());
    }
    Ok(())
}

pub fn ceil(float: f64) -> u64 {
    let int = float as u64;
    if float == int as f64 {
        return int;
    }
    return int + 1;
}

pub fn split_float(float: f64) -> (u64, f64) {
    let fl_str = float.to_string();
    let split: Vec<&str> = fl_str.split(".").collect();
    let mut frac = "0.0".to_owned();
    if split.len() == 2 {
        frac = "0.".to_owned() + split[1];
    }
    return (
        split[0].parse::<u64>().unwrap(),
        frac.parse::<f64>().unwrap(),
    );
}

pub fn check_accounts(
    instance: &HihiState,
    token_program_id: &Pubkey,
    token_mint_id: &Pubkey,
    to_info: &AccountInfo,
) -> Result<bool, ProgramError> {
    if token_program_id != &spl_token::id() {
        return Err(HihiError::InvalidOwner.into());
    }

    if token_mint_id != &instance.token_mint_id {
        return Err(HihiError::InvalidTokenMint.into());
    }

    if to_info.owner != &spl_token::id() {
        return Ok(false);
    }

    let to_account = match spl_token::state::Account::unpack_unchecked(&to_info.data.borrow()) {
        Err(_e) => return Ok(false),
        Ok(f) => f,
    };

    if to_account.mint != instance.token_mint_id {
        return Ok(false);
    }

    return Ok(true);
}
