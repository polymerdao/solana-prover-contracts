use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{read_keypair_file, Keypair};
use std::env;

mod prover_client;
use prover_client::Client;

fn default_keypair_path() -> String {
    use home::home_dir;
    use std::path::PathBuf;

    let mut path = home_dir().unwrap_or_else(|| PathBuf::from("."));
    path = path.join(".config/solana/id.json");
    path.to_string_lossy().to_string()
}

#[derive(Parser)]
#[command(name = "proverctl")]
#[command(about = "CLI for interacting with the polymer_prover program")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Solana cluster RPC endpoint (e.g., https://api.devnet.solana.com)
    #[arg(long, default_value = "http://localhost:8899")]
    cluster: String,

    /// Path to keypair file
    #[arg(long, default_value_t = default_keypair_path())]
    keypair_path: String,

    /// Keypair to sign transactions, base58 encoded
    #[arg(long, default_value = "")]
    keypair: String,

    #[arg(long)]
    program_id: Pubkey,
}

#[derive(Subcommand)]
enum Commands {
    Initialize {
        #[arg(long)]
        client_type: String,

        #[arg(long)]
        signer_addr: String,

        #[arg(long)]
        peptide_chain_id: u64,
    },
    ClearCache,
    ResizeCache,
}

fn main() -> Result<()> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let cli = Cli::parse();

    let signer = if cli.keypair != "" {
        Keypair::from_base58_string(&cli.keypair)
    } else {
        read_keypair_file(cli.keypair_path).map_err(|e| anyhow::anyhow!("Failed to read keypair: {}", e))?
    };

    let client = Client::new(cli.program_id, signer, &cli.cluster)?;
    match &cli.command {
        Commands::Initialize {
            client_type,
            signer_addr,
            peptide_chain_id,
        } => client.send_initialize(client_type, signer_addr, *peptide_chain_id)?,
        Commands::ResizeCache => client.send_resize_cache()?,
        Commands::ClearCache => client.send_clear_cache()?,
    }

    Ok(())
}
