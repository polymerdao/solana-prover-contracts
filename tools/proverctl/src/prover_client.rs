use anchor_lang::InstructionData;
use anyhow::{Ok, Result};
use log::{info, warn};
use polymer_prover::{
    instruction::{ClearProofCache, CreateAccounts, Initialize},
    instructions::parse_event::EthAddress,
};
use retry::{delay::Fixed, retry, OperationResult};
use solana_client::{rpc_client::RpcClient, rpc_config::*};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair},
    signer::Signer,
    transaction::Transaction,
};
use solana_transaction_status_client_types::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::time::Duration;

pub struct Client {
    pub program: Keypair,
    pub client: RpcClient,
    pub payer: Keypair,
}

impl Client {
    pub fn new(program_keypair: String, payer: Keypair, cluster: &str) -> Result<Self> {
        let program =
            read_keypair_file(program_keypair).map_err(|e| anyhow::anyhow!("Failed to read keypair: {}", e))?;

        let client = RpcClient::new(cluster.to_string());
        info!("PROGRAM_ID: {}", program.pubkey().to_string());
        info!("PAYER: {}", payer.pubkey());
        Ok(Client { program, payer, client })
    }

    pub fn send_initialize(&self, client_type: &String, signer_addr: &String, peptide_chain_id: u64) -> Result<()> {
        let data = polymer_prover::instruction::Initialize {
            client_type: client_type.to_string(),
            signer_addr: EthAddress::from_hex(signer_addr.as_str()).as_bytes().clone(),
            peptide_chain_id,
        };
        let (internal_account, _) = Pubkey::find_program_address(&[b"internal"], &self.program.pubkey());
        let instruction = Instruction {
            program_id: self.program.pubkey(),
            data: Initialize::data(&data),
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(self.program.pubkey(), true),
                AccountMeta::new(internal_account, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
        };

        let tx = self.send_tx(instruction, &[&self.program])?;
        self.show_tx_logs(tx);
        Ok(())
    }

    pub fn send_clear_cache(&self) -> Result<()> {
        let cache_account = self.find_cache_account();
        let instruction = Instruction {
            program_id: self.program.pubkey(),
            data: ClearProofCache.data(),
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(cache_account, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
        };

        let tx = self.send_tx(instruction, &[])?;
        self.show_tx_logs(tx);
        Ok(())
    }

    pub fn send_create_accounts(&self) -> Result<()> {
        let cache_account = self.find_cache_account();
        let instruction = Instruction {
            program_id: self.program.pubkey(),
            data: CreateAccounts.data(),
            accounts: vec![
                AccountMeta::new(self.payer.pubkey(), true),
                AccountMeta::new(cache_account, false),
                AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
        };

        let tx = self.send_tx(instruction, &[])?;
        self.show_tx_logs(tx);
        Ok(())
    }

    fn find_cache_account(&self) -> Pubkey {
        let (account, _) =
            Pubkey::find_program_address(&[b"cache", self.payer.pubkey().as_ref()], &self.program.pubkey());
        info!("CACHE: {}", account);
        account
    }

    fn show_tx_logs(&self, tx: EncodedConfirmedTransactionWithStatusMeta) {
        if let Some(meta) = tx.transaction.meta {
            let logs = meta.log_messages.unwrap();
            info!("PROGRAM LOGS:");
            for log in logs {
                info!("{}", log);
            }
        } else {
            warn!("No transaction metadata found.");
        }
    }

    fn send_tx(
        &self,
        instruction: Instruction,
        extra_signers: &[&Keypair],
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
        let recent_blockhash = self.client.get_latest_blockhash()?;
        let signers = vec![&self.payer]
            .into_iter()
            .chain(extra_signers.iter().cloned())
            .collect::<Vec<&Keypair>>();
        let tx =
            Transaction::new_signed_with_payer(&[instruction], Some(&self.payer.pubkey()), &signers, recent_blockhash);

        info!("sending transaction...");
        let config = RpcSendTransactionConfig {
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            ..Default::default()
        };
        let sig = self.client.send_transaction_with_config(&tx, config)?;

        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Json),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        info!("got transaction signature: {}", sig);

        // retry 10 times, 1 second each
        let delay = Fixed::from(Duration::from_secs(1)).take(10);
        let result = retry(delay, || match self.client.get_transaction_with_config(&sig, config) {
            std::result::Result::Ok(val) => OperationResult::Ok(val),
            std::result::Result::Err(err) => OperationResult::Retry(err),
        });

        Ok(result?)
    }
}
