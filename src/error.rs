//! error are specific errors in our custom program

use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    msg,
    program_error::{PrintProgramError, ProgramError},
};
use std::fmt;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum HihiError {
    InvalidInstruction,
    InvalidOwner,
    InvalidProgramAddress,
    InvalidTokenMint,
    InvalidTokenAddress,
    WorkLimitExceeded,
    InvalidClaimHash,
    IncorrectClaimSolution,
    DeserializationFailure,
    NotInitialized,
    AlreadyInitialized,
    NotRentExempt,
    InsufficientFundsForTransaction,
    UnknownError,
}

impl From<HihiError> for ProgramError {
    fn from(e: HihiError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for HihiError {
    fn type_of() -> &'static str {
        "HihiError"
    }
}

impl fmt::Display for HihiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HihiError::InvalidInstruction => f.write_str("Invalid instruction"),
            HihiError::InvalidOwner => f.write_str("Invalid owner"),
            HihiError::InvalidProgramAddress => f.write_str("Invalid program address"),
            HihiError::InvalidTokenMint => f.write_str("Invalid token mint address"),
            HihiError::InvalidTokenAddress => f.write_str("Invalid token address"),
            HihiError::WorkLimitExceeded => f.write_str("Work limit exceeded"),
            HihiError::InvalidClaimHash => f.write_str("Invalid claim hash"),
            HihiError::IncorrectClaimSolution => f.write_str("Incorrect claim solution"),
            HihiError::DeserializationFailure => f.write_str("Error Deserializing input data"),
            HihiError::NotInitialized => f.write_str("Account not initialized"),
            HihiError::AlreadyInitialized => f.write_str("Account already initialized"),
            HihiError::NotRentExempt => f.write_str("Account must be rent exempt"),
            HihiError::UnknownError => f.write_str("Unknown error condiiton"),
            HihiError::InsufficientFundsForTransaction => {
                f.write_str("Not enough funds to process transaction")
            }
        }
    }
}

impl PrintProgramError for HihiError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            HihiError::InvalidInstruction => msg!("Error: Invalid instruction"),
            HihiError::InvalidOwner => msg!("Error: Invalid owner"),
            HihiError::InvalidProgramAddress => msg!("Invalid program address"),
            HihiError::InvalidTokenMint => msg!("Invalid token mint address"),
            HihiError::InvalidTokenAddress => msg!("Invalid token address"),
            HihiError::WorkLimitExceeded => msg!("Work limit exceeded"),
            HihiError::InvalidClaimHash => msg!("Invalid claim hash"),
            HihiError::IncorrectClaimSolution => msg!("Incorrect claim solution"),
            HihiError::DeserializationFailure => msg!("Error Deserializing input data"),
            HihiError::NotInitialized => msg!("Account not initialized"),
            HihiError::AlreadyInitialized => msg!("Account already initialized"),
            HihiError::NotRentExempt => msg!("Account must be rent exempt"),
            HihiError::UnknownError => msg!("Unknown error condiiton"),
            HihiError::InsufficientFundsForTransaction => {
                msg!("Not enough funds to process transaction")
            }
        }
    }
}
