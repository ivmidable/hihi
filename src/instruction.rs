use crate::error::HihiError;
use std::convert::TryFrom;
use std::convert::TryInto;

use arrayref::array_ref;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar,
};
use std::mem::size_of;

pub const WORK_BYTES: usize = 57;

#[derive(Clone, Debug, PartialEq)]
pub struct Initialize {
    pub nonce: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Breach {
    pub lamports: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub work: [u8; WORK_BYTES],
}

#[derive(Debug, PartialEq)]
/// All custom program instructions
pub enum HihiInstruction {
    Initialize(Initialize),
    Breach(Breach),
    LimitBreak,
    Claim(Claim),
    Withdraw,
    ChangeKeys,
}

impl HihiInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(HihiError::InvalidInstruction)?;
        match tag {
            0 => {
                let (nonce, _rest) = rest.split_at(1);
                Ok(Self::Initialize(Initialize { nonce: nonce[0] }))
            }
            1 => {
                let (lamports, _rest) = Self::unpack_u64(rest)?;
                Ok(Self::Breach(Breach { lamports: lamports }))
            }
            2 => Ok(HihiInstruction::LimitBreak),
            3 => {
                let (work, _rest) = Self::unpack_work(rest)?;
                Ok(Self::Claim(Claim { work: work }))
            }
            4 => Ok(HihiInstruction::Withdraw),
            5 => Ok(HihiInstruction::ChangeKeys),
            _ => Err(HihiError::DeserializationFailure.into()),
        }
    }

    fn unpack_work(input: &[u8]) -> Result<([u8; WORK_BYTES], &[u8]), ProgramError> {
        if input.len() >= WORK_BYTES {
            let (work, rest) = input.split_at(WORK_BYTES);
            //let _w = work.to_vec();
            let w = <[u8; WORK_BYTES]>::try_from(<&[u8]>::clone(&work))
                .expect("Slice must be the same length as [u8; WORK_BYTES](49).");
            Ok((w, rest))
        } else {
            Err(HihiError::InvalidInstruction.into())
        }
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(HihiError::InvalidInstruction)?;
            Ok((amount, rest))
        } else {
            Err(HihiError::InvalidInstruction.into())
        }
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::<u8>::with_capacity(size_of::<Self>());
        match &*self {
            Self::Initialize(Initialize { nonce }) => {
                buf.push(0);
                buf.push(*nonce);
            }
            Self::Breach(Breach { lamports }) => {
                buf.push(1);
                buf.extend_from_slice(&lamports.to_le_bytes());
            }
            Self::LimitBreak => {
                buf.push(2);
            }
            Self::Claim(Claim { work }) => {
                buf.push(3);
                buf.extend_from_slice(array_ref!(work, 0, WORK_BYTES));
            }
            Self::Withdraw => {
                buf.push(4);
            }
            Self::ChangeKeys => {
                buf.push(5);
            }
        }
        buf
    }
}

//Instructions

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    instance_id: &Pubkey,
    initializer_id: &Pubkey,
    token_mint_id: &Pubkey,
    admin_one_id: &Pubkey,
    admin_two_id: &Pubkey,
    withdraw_id: &Pubkey,
    nonce: u8,
) -> Result<Instruction, ProgramError> {
    let data = HihiInstruction::Initialize(Initialize { nonce }).pack();

    let accounts = vec![
        AccountMeta::new(*instance_id, true),
        AccountMeta::new_readonly(*initializer_id, true),
        AccountMeta::new_readonly(*token_mint_id, false),
        AccountMeta::new_readonly(*admin_one_id, true),
        AccountMeta::new_readonly(*admin_two_id, true),
        AccountMeta::new_readonly(*withdraw_id, true),
        AccountMeta::new_readonly(sysvar::rent::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

// breach instruction.
pub fn breach(
    program_id: &Pubkey,
    instance_id: &Pubkey,
    token_program_id: &Pubkey,
    token_mint_id: &Pubkey,
    authority_id: &Pubkey,
    to_token: &Pubkey,
    from_id: &Pubkey,
    //to_lamps: &Pubkey,
    lamports: u64,
) -> Result<Instruction, ProgramError> {
    let data = HihiInstruction::Breach(Breach { lamports }).pack();

    let accounts = vec![
        AccountMeta::new(*instance_id, false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*token_mint_id, false),
        AccountMeta::new(*authority_id, false),
        AccountMeta::new(*from_id, true),
        AccountMeta::new(*to_token, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn limit_break(
    program_id: &Pubkey,
    instance_id: &Pubkey,
    token_program_id: &Pubkey,
    token_mint_id: &Pubkey,
    authority_id: &Pubkey,
    to_token: &Pubkey,
    to_lamports: &Pubkey,
    claim_key: &Pubkey,
    pool_key: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = HihiInstruction::LimitBreak.pack();

    let accounts = vec![
        AccountMeta::new(*instance_id, false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*token_mint_id, false),
        AccountMeta::new(*authority_id, false),
        AccountMeta::new_readonly(*claim_key, true),
        AccountMeta::new_readonly(*pool_key, true),
        AccountMeta::new(*to_token, false),
        AccountMeta::new(*to_lamports, false),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn claim(
    program_id: &Pubkey,
    instance_id: &Pubkey,
    token_program_id: &Pubkey,
    token_mint_id: &Pubkey,
    authority_id: &Pubkey,
    claim_pubkey: &Pubkey,
    pool_pubkey: &Pubkey,
    to_pubkey: &Pubkey,
    work: [u8; WORK_BYTES],
) -> Result<Instruction, ProgramError> {
    let data = HihiInstruction::Claim(Claim { work }).pack();

    let accounts = vec![
        AccountMeta::new(*instance_id, false),
        AccountMeta::new_readonly(*token_program_id, false),
        AccountMeta::new(*token_mint_id, false),
        AccountMeta::new_readonly(*authority_id, false),
        AccountMeta::new_readonly(*claim_pubkey, true),
        AccountMeta::new_readonly(*pool_pubkey, true),
        AccountMeta::new(*to_pubkey, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn withdraw(
    program_id: &Pubkey,
    instance_id: &Pubkey,
    authority_id: &Pubkey,
    withdraw_id: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = HihiInstruction::ChangeKeys.pack();

    let accounts = vec![
        AccountMeta::new_readonly(*instance_id, false),
        AccountMeta::new(*authority_id, false),
        AccountMeta::new(*withdraw_id, true),
        AccountMeta::new_readonly(solana_program::system_program::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

pub fn change_keys(
    program_id: &Pubkey,
    instance_id: &Pubkey,
    admin_one_key: &Pubkey,
    admin_two_key: &Pubkey,
    withdraw_key: &Pubkey,
    new_admin_one_key: &Pubkey,
    new_admin_two_key: &Pubkey,
    new_withdraw_key: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = HihiInstruction::ChangeKeys.pack();

    let accounts = vec![
        AccountMeta::new(*instance_id, false),
        AccountMeta::new_readonly(*admin_one_key, true),
        AccountMeta::new_readonly(*admin_two_key, true),
        AccountMeta::new_readonly(*withdraw_key, true),
        AccountMeta::new_readonly(*new_admin_one_key, true),
        AccountMeta::new_readonly(*new_admin_two_key, true),
        AccountMeta::new_readonly(*new_withdraw_key, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}
