use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Initialize>,
    client_type: String,
    signer_addr: [u8; 20],
    peptide_chain_id: u64,
) -> Result<()> {
    Ok(())
}
