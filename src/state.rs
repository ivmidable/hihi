//Account State
use crate::error::HihiError;

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};

use solana_program::{
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_memory::sol_memcpy,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

pub const TOKENS: usize = 1;
pub const WORK: usize = 32;
pub const MAGIC_LEN:usize = 1;
pub const MAGIC: usize = 23;
pub const WORK_BYTES: usize = TOKENS + WORK + MAGIC_LEN + MAGIC;
pub const LB_BYTES: usize = TOKENS + WORK + MAGIC_LEN + MAGIC;
pub const LB_COUNT_BYTES: usize = 4;
pub const LB_PER_EPOCH_BYTES: usize = 4;

pub const MAX_COUNT: usize = 101;

pub const INITIALIZED_BYTES: usize = 1;
pub const NONCE_BYTES: usize = 1;
pub const SLOT_BYTES: usize = 8;
pub const EPOCH_BYTES: usize = 8;
pub const DIFFICULTY_BYTES: usize = 1;
pub const LAMPORTS_BYTES:usize = 8;
pub const PRICE_BYTES:usize = 8;
pub const REMAIN_BYTES:usize = 8;
pub const COUNT_BYTES: usize = 4;
pub const COUNT_PER_WINDOW_BYTES: usize = 4;

pub const CACHED_BYTES: usize = 8;
pub const TOKEN_MINT_ID_BYTES: usize = 32;
pub const TOKEN_DOUBLES_BYTES: usize = 8;

pub const ADMIN_ONE_BYTES: usize = 32;
pub const ADMIN_TWO_BYTES: usize = 32;
pub const WITHDRAW_BYTES: usize = 32;

pub const VEC_COUNT: usize = 1;
pub const VEC_DATA_LENGTH: usize = 4;
pub const VEC_DATA: usize = WORK_BYTES*MAX_COUNT;
pub const STATE_SPACE: usize = INITIALIZED_BYTES + NONCE_BYTES + SLOT_BYTES + EPOCH_BYTES + DIFFICULTY_BYTES + LAMPORTS_BYTES + PRICE_BYTES + REMAIN_BYTES + COUNT_BYTES + COUNT_PER_WINDOW_BYTES + CACHED_BYTES + TOKEN_MINT_ID_BYTES + TOKEN_DOUBLES_BYTES + LB_COUNT_BYTES + LB_PER_EPOCH_BYTES + LB_BYTES + ADMIN_ONE_BYTES + ADMIN_TWO_BYTES + WITHDRAW_BYTES + VEC_COUNT + VEC_DATA_LENGTH + VEC_DATA;

#[derive(Debug, PartialEq)]
pub struct HihiState {
    pub is_initialized: bool,
    pub token_mint_id: Pubkey,
    pub token_doubles:u64,
    pub nonce:u8,
    pub current_slot:u64,
    pub current_epoch:u64,
    pub difficulty:u8,
    pub lamports:u64,
    pub breach_price:u64,
    pub breach_remain:u64,
    pub breach_count:i32,
    pub breach_count_this_window:u32,
    pub limit_count:u32,
    pub limit_breaks_this_epoch:u32,
    pub admin_one_id: Pubkey,
    pub admin_two_id: Pubkey,
    pub withdraw_id: Pubkey,
    pub limit_break:Vec<u8>,
    pub work_cached:u64,
    pub work: Vec<Vec<u8>>
}

impl HihiState {
    pub fn set_initialized(&mut self) {
        self.is_initialized = true;
    }

    pub fn add_work(&mut self, work: &[u8]) -> Result<(), HihiError> {
        let count = work.len()/WORK_BYTES;
        let mut pos = 0;
        if self.work.len()+count <= MAX_COUNT {
            for _ in 0..count {
                let w = &work[pos..pos+WORK_BYTES];
                self.work.push(w.to_vec());
                pos+=WORK_BYTES;
            }
            Ok(())
        } else {
            Err(HihiError::WorkLimitExceeded)
        }
    }

    //make sure the index is valid before calling this.
    pub fn remove_work(&mut self, index: usize) -> ProgramResult {
        self.work.swap_remove(index);
        Ok(())
    }

    pub fn get_work_bytes() -> usize {
        return WORK_BYTES;
    }

    pub fn get_work_free_space(&self) -> i32 {
        return (MAX_COUNT - self.work.len()-1) as i32;
    }

    pub fn get_space(&self) -> usize {
        return STATE_SPACE;
    }
}

impl Sealed for HihiState {}

impl IsInitialized for HihiState {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for HihiState {
    const LEN: usize = STATE_SPACE;

    fn pack_into_slice(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, STATE_SPACE];
        let (
            is_initialized,
            nonce,
            current_slot,
            current_epoch,
            difficulty,
            lamports,
            breach_price,
            breach_remain,
            breach_count,
            breach_count_this_window,
            work_cached,
            token_mint_id,
            token_doubles,
            limit_count,
            limit_breaks_this_epoch,
            admin_one_id,
            admin_two_id,
            withdraw_id,
            limit_break,
            vec_count,
            vec_data_length,
            vec_data
        ) = mut_array_refs![output, INITIALIZED_BYTES, NONCE_BYTES, SLOT_BYTES, EPOCH_BYTES, DIFFICULTY_BYTES, LAMPORTS_BYTES, PRICE_BYTES, REMAIN_BYTES, COUNT_BYTES, COUNT_PER_WINDOW_BYTES, CACHED_BYTES, TOKEN_MINT_ID_BYTES, TOKEN_DOUBLES_BYTES, LB_COUNT_BYTES, LB_PER_EPOCH_BYTES, ADMIN_ONE_BYTES , ADMIN_TWO_BYTES , WITHDRAW_BYTES, LB_BYTES, VEC_COUNT, VEC_DATA_LENGTH, VEC_DATA];
        is_initialized[0] = self.is_initialized as u8;
        nonce[0] = self.nonce as u8;
        current_slot[..].copy_from_slice(&self.current_slot.to_le_bytes());
        current_epoch[..].copy_from_slice(&self.current_epoch.to_le_bytes());
        difficulty[0] = self.difficulty as u8;
        lamports[..].copy_from_slice(&self.lamports.to_le_bytes());
        breach_price[..].copy_from_slice(&self.breach_price.to_le_bytes());
        breach_remain[..].copy_from_slice(&self.breach_remain.to_le_bytes());
        breach_count[..].copy_from_slice(&self.breach_count.to_le_bytes());
        breach_count_this_window[..].copy_from_slice(&self.breach_count_this_window.to_le_bytes());
        work_cached[..].copy_from_slice(&self.work_cached.to_le_bytes());
        token_mint_id.copy_from_slice(self.token_mint_id.as_ref());
        token_doubles[..].copy_from_slice(&self.token_doubles.to_le_bytes());
        limit_count[..].copy_from_slice(&self.limit_count.to_le_bytes());
        limit_breaks_this_epoch[..].copy_from_slice(&self.limit_breaks_this_epoch.to_le_bytes());
        admin_one_id.copy_from_slice(self.admin_one_id.as_ref());
        admin_two_id.copy_from_slice(self.admin_two_id.as_ref());
        withdraw_id.copy_from_slice(self.withdraw_id.as_ref());
        sol_memcpy(limit_break, &self.limit_break, LB_BYTES);
        vec_count[0] = self.work.len() as u8;
        let data = pack_vec_of_vec(&self.work);
        let data_len = data.len();
        if data_len < VEC_DATA {
            vec_data_length[..].copy_from_slice(&(data_len as u32).to_le_bytes());
            sol_memcpy(vec_data, &data, data_len);
        } else {
            panic!("Not allowed to excede {} pow account limit.", MAX_COUNT);
        }
    }

    fn unpack_from_slice(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, STATE_SPACE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (
            is_initialized,
            nonce,
            current_slot,
            current_epoch,
            difficulty,
            lamports,
            breach_price,
            breach_remain,
            breach_count,
            breach_count_this_window,
            work_cached,
            token_mint_id,
            token_doubles,
            limit_count,
            limit_breaks_this_epoch,
            admin_one_id,
            admin_two_id,
            withdraw_id,
            limit_break,
            vec_count,
            _vec_data_length,
            vec_data
        ) = array_refs![input, INITIALIZED_BYTES, NONCE_BYTES, SLOT_BYTES, EPOCH_BYTES, DIFFICULTY_BYTES, LAMPORTS_BYTES, PRICE_BYTES, REMAIN_BYTES, COUNT_BYTES, COUNT_PER_WINDOW_BYTES, CACHED_BYTES, TOKEN_MINT_ID_BYTES, TOKEN_DOUBLES_BYTES, LB_COUNT_BYTES, LB_PER_EPOCH_BYTES, ADMIN_ONE_BYTES , ADMIN_TWO_BYTES , WITHDRAW_BYTES, LB_BYTES, VEC_COUNT, VEC_DATA_LENGTH, VEC_DATA];

        let is_init = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData)
        };
        
        if is_init == false {
            Ok(Self{
                is_initialized:is_init,
                nonce:0,
                current_slot:0,
                current_epoch:0,
                difficulty:0,
                lamports:0,
                breach_price:0,
                breach_remain:0,
                breach_count:0,
                breach_count_this_window:0,
                work_cached:0,
                token_mint_id:Pubkey::new_from_array(*token_mint_id),
                token_doubles:0,
                limit_count:0,
                limit_breaks_this_epoch:0,
                admin_one_id:Pubkey::new_from_array(*admin_one_id),
                admin_two_id:Pubkey::new_from_array(*admin_two_id),
                withdraw_id:Pubkey::new_from_array(*withdraw_id),
                limit_break:Vec::<u8>::new(),
                work:Vec::<Vec<u8>>::new()
            })
        } else {
            Ok(Self {
                is_initialized:is_init,
                nonce:nonce[0],
                current_slot:u64::from_le_bytes(*current_slot),
                current_epoch:u64::from_le_bytes(*current_epoch),
                difficulty:difficulty[0],
                lamports:u64::from_le_bytes(*lamports),
                breach_price:u64::from_le_bytes(*breach_price),
                breach_remain:u64::from_le_bytes(*breach_remain),
                breach_count:i32::from_le_bytes(*breach_count),
                breach_count_this_window:u32::from_le_bytes(*breach_count_this_window),
                work_cached:u64::from_le_bytes(*work_cached),
                token_mint_id:Pubkey::new_from_array(*token_mint_id),
                token_doubles:u64::from_le_bytes(*token_doubles),
                limit_count:u32::from_le_bytes(*limit_count),
                limit_breaks_this_epoch:u32::from_le_bytes(*limit_breaks_this_epoch),
                admin_one_id:Pubkey::new_from_array(*admin_one_id),
                admin_two_id:Pubkey::new_from_array(*admin_two_id),
                withdraw_id:Pubkey::new_from_array(*withdraw_id),
                limit_break:limit_break.to_vec(),
                work:unpack_vec_of_vec(&vec_data, vec_count[0])
            })
        }
    }
}

fn pack_vec_of_vec(args: &Vec<Vec<u8>>) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    for v in args {
        buf.extend_from_slice(array_ref![v.as_slice(), 0, WORK_BYTES]);
    }
    return buf;
}

fn unpack_vec_of_vec(slice: &[u8; VEC_DATA], count:u8) -> Vec<Vec<u8>> {
    let mut buf = Vec::<Vec<u8>>::new();
    let mut m_rest:&[u8] = slice;
    for _ in 0..count {
        let (work, rest) = m_rest.split_at(WORK_BYTES);
        buf.push(work.to_vec());
        m_rest = rest;
    }
    return buf;
}
