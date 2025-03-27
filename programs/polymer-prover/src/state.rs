use anchor_lang::prelude::*;

#[account]
pub struct EventAccount {
    /// Client type used on peptide to generate the proof. It is part of the proof key
    pub client_type: String,

    /// Known signer address that signed the peptide state root
    pub signer_addr: [u8; 20],

    // Peptide chain ID included in the proof
    pub peptide_chain_id: u64,
}

impl EventAccount {
    pub const MAX_SIZE: usize = 4 + 100 + 20 + 8; // String (dynamic), array, and u64 storage
}
