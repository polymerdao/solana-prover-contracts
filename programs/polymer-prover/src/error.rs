use anchor_lang::prelude::*;

#[error_code]
pub enum ProverError {
    // TODO add the reason why it's invalid
    #[msg("The provided signature is invalid.")]
    InvalidSignature,

    #[msg("The signer is unknown.")]
    UnknownSigner,

    // TODO add the reason why it's invalid
    #[msg("The provided address is invalid.")]
    InvalidAddress,
}
