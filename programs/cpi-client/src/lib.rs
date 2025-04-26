#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::get_return_data, sysvar};
use borsh::BorshDeserialize;
use polymer_prover::program::PolymerProver;

declare_id!("qTdgaLBq1tWoTx81t3rZ1mDBTdJ7GvyPqnfVxSPv2sz");

#[derive(Accounts)]
#[instruction()]
pub struct CallLoadProof<'info> {
    /// CHECK: PDA will be created in the callee if needed
    #[account(mut)]
    pub cache_account: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub polymer_prover: Program<'info, PolymerProver>,
}

#[derive(Accounts)]
#[instruction()]
pub struct CallValidateEvent<'info> {
    /// CHECK: PDA will be created in the callee if needed
    #[account(mut)]
    pub cache_account: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub polymer_prover: Program<'info, PolymerProver>,

    /// CHECK: PDA will be created in the callee if needed
    pub internal: UncheckedAccount<'info>,
    /// CHECK: PDA will be created in the callee if needed
    #[account(address = sysvar::instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
}

/// this simple program is meant to be used only for testing the CPI capabilities of our
/// polymer_prover program
/// It exposes two instrctions: call_load_proof and call_validate_event, that simply call into
/// their cousins on the prover
/// For simplicity, the proof is loaded into the binary itself and not provided by the caller.
#[program]
pub mod cpi_client {

    use super::*;

    use hex::FromHex;
    use polymer_prover::cpi::accounts::LoadProof as PolymerLoadProof;
    use polymer_prover::cpi::accounts::ValidateEvent as PolymerValidateEvent;
    use polymer_prover::instructions::validate_event::ValidateEventResult;

    // load the proof here for simplicity
    const PROOF_HEX: &str = include_str!("../../polymer-prover/src/instructions/test-data/op-proof-small.hex");

    /// calls load_proof on the polymer_prover program with a pre-loaded proof
    pub fn call_load_proof(ctx: Context<CallLoadProof>) -> Result<()> {
        msg!("calling load_proof from cpi_client");

        let proof = Vec::from_hex(&PROOF_HEX.trim_start_matches("0x")).expect("could not load proof?");

        polymer_prover::cpi::load_proof(
            CpiContext::new(
                ctx.accounts.polymer_prover.to_account_info(),
                PolymerLoadProof {
                    cache_account: ctx.accounts.cache_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                },
            ),
            proof,
        )
    }

    /// calls validate_event() on the polymer_prover program and checks the returned data
    pub fn call_validate_event(ctx: Context<CallValidateEvent>) -> Result<()> {
        msg!("calling validate_event from cpi_client");

        polymer_prover::cpi::validate_event(CpiContext::new(
            ctx.accounts.polymer_prover.to_account_info(),
            PolymerValidateEvent {
                cache_account: ctx.accounts.cache_account.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
                internal: ctx.accounts.internal.to_account_info(),
                instructions: ctx.accounts.instructions.to_account_info(),
            },
        ))?;

        let (pid, data) = get_return_data().ok_or(ErrorCode::MissingReturn)?;
        require_keys_eq!(pid, polymer_prover::ID, ErrorCode::WrongProgram);

        let result = ValidateEventResult::try_from_slice(&data)?;
        match result {
            ValidateEventResult::Valid(chain_id, event) => {
                // don't bother emitting all the parsed event. Just print something here so we can
                // assert on the test
                msg!(
                    "proof validated: chain_id: {}, emitting_contract: {}",
                    chain_id,
                    event.emitting_contract.to_hex()
                )
            }
            _ => msg!("prover returned error: {}", result),
        }

        Ok(())
    }
}

#[error_code]
pub enum ErrorCode {
    #[msg("The return data was not from the expected program.")]
    WrongProgram,

    #[msg("No return data was set.")]
    MissingReturn,
}
