use std::usize;

use crate::ProverError;
use anchor_lang::solana_program::{keccak, secp256k1_recover::secp256k1_recover};
use sha2::{Digest, Sha256};
use sha3::Keccak256;

#[derive(Debug, PartialEq, Clone)]
pub struct EthAddress([u8; 20]);

impl EthAddress {
    pub fn from_bytes(bytes: &[u8; 20]) -> Result<Self, ()> {
        Ok(EthAddress(*bytes))
    }

    /// Converts a hex string (with or without "0x" prefix) into an EthAddress.
    pub fn from_hex(str: &str) -> Result<Self, ()> {
        let mut addr = [0u8; 20];
        let _ = hex::decode_to_slice(str.trim_start_matches("0x"), &mut addr as &mut [u8]);
        Ok(EthAddress(addr))
    }

    /// Returns the address as a byte array.
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_string(&self) -> String {
        hex::encode(&self.0)
    }
}

pub fn handler(
    client_type: &String,
    signer_addr: &[u8; 20],
    peptide_chain_id: u64,
    proof: Vec<u8>,
) -> Result<(), ProverError> {
    let eth_addr = EthAddress::from_bytes(signer_addr).unwrap();

    println!("client type: {}", client_type);
    println!("signer addr: {:?}", eth_addr.to_string());
    println!("chain id:    {}", peptide_chain_id);

    let app_hash = &<[u8; 32]>::try_from(&proof[0..32]).unwrap();

    if !verify_signature(
        &eth_addr,
        peptide_chain_id,
        app_hash,
        &<[u8; 8]>::try_from(&proof[101..109]).unwrap(),
        &<[u8; 64]>::try_from(&proof[32..96]).unwrap(),
        proof[96],
    ) {
        return Err(ProverError::InvalidSignature.into());
    }

    let event_end: usize =
        u16::from_be_bytes(<[u8; 2]>::try_from(&proof[121..123]).unwrap()).into();

    let key = format!(
        "chain/{}/storedLogs/{}/{}/{}/{}",
        u32::from_be_bytes(<[u8; 4]>::try_from(&proof[97..101]).unwrap()),
        client_type,
        u64::from_be_bytes(<[u8; 8]>::try_from(&proof[109..117]).unwrap()),
        u16::from_be_bytes(<[u8; 2]>::try_from(&proof[117..119]).unwrap()),
        proof[119],
    );

    println!("key {}", key);
    let value = {
        let mut hasher = keccak::Hasher::default();
        hasher.hash(&proof[123..event_end]);
        hasher.result()
    };

    if !verify_membership(app_hash, key.as_bytes(), &value.0, &proof[event_end..]) {
        return Err(ProverError::InvalidSignature.into());
    }

    Ok(())
}

fn verify_signature(
    signer_addr: &EthAddress,
    peptide_chain_id: u64,
    app_hash: &[u8; 32],
    peptide_height: &[u8; 8],
    signature: &[u8; 64],
    recovery_id: u8,
) -> bool {
    let message_hash = {
        let mut hasher = keccak::Hasher::default();
        hasher.hash(app_hash);
        hasher.hash(peptide_height);
        hasher.result()
    };

    let hash = {
        let mut hasher = keccak::Hasher::default();
        hasher.hash(&[0; 32]);
        hasher.hash(&u64_to_32_bytes_array(peptide_chain_id));
        hasher.hash(&message_hash.0);
        hasher.result()
    };

    if let Ok(recovered_pubkey) = secp256k1_recover(&hash.0, recovery_id - 27, signature) {
        let recovered_address_hash = Keccak256::digest(recovered_pubkey.to_bytes());
        // take last 20 bytes
        *signer_addr.as_bytes() == recovered_address_hash[12..32]
    } else {
        false
    }
}

fn verify_membership(app_hash: &[u8; 32], key: &[u8], value: &[u8; 32], proof: &[u8]) -> bool {
    let number_of_paths: usize = proof[0].into();
    let path_zero_start: usize = proof[1].into();

    let hashed_value = {
        let mut hasher = Sha256::new();
        hasher.update(value);
        hasher.finalize()
    };

    println!("value        {:?}", hex::encode(value));
    println!("hashed_value {:?}", hex::encode(hashed_value));

    let mut pre_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&proof[2..path_zero_start]);
        hasher.update(key);
        hasher.update([32u8; 1]);
        hasher.update(hashed_value);
        hasher.finalize()
    };

    let mut offset: usize = path_zero_start;
    for _ in 0..number_of_paths {
        let suffix_start: usize = proof[offset].into();
        let suffix_end: usize = proof[offset + 1].into();

        let mut hasher = Sha256::new();
        hasher.update(&proof[offset + 2..offset + suffix_start]);
        hasher.update(pre_hash);
        hasher.update(&proof[offset + suffix_start..offset + suffix_end]);
        pre_hash = hasher.finalize();
        offset = offset + suffix_end;
    }

    println!("pre_hash {:?}", hex::encode(pre_hash.as_slice()));
    println!("app_hash {:?}", hex::encode(app_hash));
    pre_hash.as_slice() == *app_hash
}

fn u64_to_32_bytes_array(input: u64) -> [u8; 32] {
    let mut result = [0u8; 32];
    let bytes = input.to_be_bytes();
    // Copy the 8-byte array into the last 8 bytes of the 32-byte array
    result[24..].copy_from_slice(&bytes);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex;
    use std::fs::File;
    use std::io::Read;

    #[test]
    fn test_validate_event() {
        let proof = read_and_decode_proof_file("src/instructions/test-data/op-proof-v2.hex")
            .expect("could not read proof file");

        let client_type = "proof_api";
        let addr = EthAddress::from_hex("8D3921B96A3815F403Fb3a4c7fF525969d16f9E0").unwrap();
        let peptide_chain_id = 901;

        handler(
            &client_type.to_string(),
            addr.as_bytes(),
            peptide_chain_id,
            proof,
        )
        .expect("invalid proof");
    }

    fn read_and_decode_proof_file(file_path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let decoded = hex::decode(&contents.trim().as_bytes()[2..])?;
        Ok(decoded)
    }
}
