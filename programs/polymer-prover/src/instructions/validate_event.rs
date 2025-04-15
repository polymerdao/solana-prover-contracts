use crate::instructions::parse_event::{self, EthAddress};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{keccak, secp256k1_recover::secp256k1_recover};
use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use std::fmt;

use super::parse_event::EthEvent;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum ValidateEventResult {
    InvalidSignature(String),

    InvalidProof(usize, usize),

    InvalidMembershipProof(String),

    InvalidStateRoot(Vec<u8>),

    RecoveredInvalidSignerAddress(EthAddress),

    Valid(u32, EthEvent),
}

impl fmt::Display for ValidateEventResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidateEventResult::InvalidProof(got, needed) => {
                write!(
                    f,
                    "invalid proof: got {} bytes, at least {} are needed",
                    got, needed
                )
            }

            ValidateEventResult::InvalidSignature(err_msg) => {
                write!(f, "invalid signature: {}", err_msg)
            }

            ValidateEventResult::InvalidStateRoot(state_root) => {
                write!(f, "invalid state root: 0x{}", hex::encode(state_root))
            }

            ValidateEventResult::InvalidMembershipProof(err_msg) => {
                write!(f, "invalid membership proof: {}", err_msg)
            }

            ValidateEventResult::RecoveredInvalidSignerAddress(recovered) => {
                write!(
                    f,
                    "recovered invalid signer address: {}",
                    recovered.to_hex()
                )
            }

            ValidateEventResult::Valid(..) => {
                write!(f, "proof is valid")
            }
        }
    }
}

pub fn handler(
    proof: &Vec<u8>,
    client_type: &String,
    signer_addr: &[u8; 20],
    peptide_chain_id: u64,
) -> ValidateEventResult {
    // first, check there's enough data to read the event_end index
    let proof_len = proof.len();
    if proof_len < 123 {
        return ValidateEventResult::InvalidProof(proof_len, 123);
    }

    let event_end: usize =
        u16::from_be_bytes(<[u8; 2]>::try_from(&proof[121..123]).unwrap()).into();

    // now, make sure we have enough data to read until the event ends. After it, we have the
    // membership proof, which will be checked later
    if proof_len < event_end {
        return ValidateEventResult::InvalidProof(proof_len, event_end);
    }

    let app_hash = &<[u8; 32]>::try_from(&proof[0..32]).unwrap();

    let recovered = recover_signature(
        peptide_chain_id,
        app_hash,
        &<[u8; 8]>::try_from(&proof[101..109]).unwrap(),
        &<[u8; 64]>::try_from(&proof[32..96]).unwrap(),
        proof[96],
    );
    if let Err(err) = recovered {
        return ValidateEventResult::InvalidSignature(err.to_string());
    }
    if let Ok(addr) = &recovered {
        if addr.as_bytes() != signer_addr {
            return ValidateEventResult::RecoveredInvalidSignerAddress(*addr);
        }
    }

    let chain_id = u32::from_be_bytes(<[u8; 4]>::try_from(&proof[97..101]).unwrap());
    let key = format!(
        "chain/{}/storedLogs/{}/{}/{}/{}",
        chain_id,
        client_type,
        u64::from_be_bytes(<[u8; 8]>::try_from(&proof[109..117]).unwrap()),
        u16::from_be_bytes(<[u8; 2]>::try_from(&proof[117..119]).unwrap()),
        proof[119],
    );

    let value = {
        let mut hasher = keccak::Hasher::default();
        hasher.hash(&proof[123..event_end]);
        hasher.result()
    };

    if let Some(err) = verify_membership(app_hash, key.as_bytes(), &value.0, &proof[event_end..]) {
        return err;
    }

    let raw_event = &proof[123..event_end];
    let num_topics: usize = proof[120].into();

    let eth_event = parse_event::handler(raw_event, num_topics);

    ValidateEventResult::Valid(chain_id, eth_event)
}

fn recover_signature(
    peptide_chain_id: u64,
    app_hash: &[u8; 32],
    peptide_height: &[u8; 8],
    signature: &[u8; 64],
    recovery_id: u8,
) -> std::result::Result<EthAddress, String> {
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

    match secp256k1_recover(&hash.0, recovery_id - 27, signature) {
        Ok(recovered_pubkey) => {
            let recovered_address_hash = Keccak256::digest(recovered_pubkey.to_bytes());
            // take last 20 bytes
            Ok(EthAddress::from_bytes(&recovered_address_hash[12..32]))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn verify_membership(
    app_hash: &[u8; 32],
    key: &[u8],
    value: &[u8; 32],
    proof: &[u8],
) -> Option<ValidateEventResult> {
    // first make sure we have enough data to read the start of the first path
    if proof.len() < 2 {
        return Some(ValidateEventResult::InvalidMembershipProof(
            "can't read start of first path".to_string(),
        ));
    }

    let number_of_paths: usize = proof[0].into();
    let path_zero_start: usize = proof[1].into();

    // add 1 here to cover for the first suffix_end read down below
    if proof.len() < path_zero_start + 1 {
        return Some(ValidateEventResult::InvalidMembershipProof(
            "can't read first path".to_string(),
        ));
    }

    let hashed_value = {
        let mut hasher = Sha256::new();
        hasher.update(value);
        hasher.finalize()
    };

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

        if proof.len() < offset + suffix_end {
            return Some(ValidateEventResult::InvalidMembershipProof(
                "can't read path".to_string(),
            ));
        }
        let mut hasher = Sha256::new();
        hasher.update(&proof[offset + 2..offset + suffix_start]);
        hasher.update(pre_hash);
        hasher.update(&proof[offset + suffix_start..offset + suffix_end]);
        pre_hash = hasher.finalize();
        offset = offset + suffix_end;
    }

    if pre_hash.as_slice() != *app_hash {
        return Some(ValidateEventResult::InvalidStateRoot(pre_hash.to_vec()));
    }

    return None;
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
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Event {
        address: String,
        data: String,
        topics: Vec<String>,
    }

    struct TestContext {
        proof: Vec<u8>,
        event: Event,
        client_type: String,
        signer: EthAddress,
        peptide_chain_id: u64,
    }

    impl Drop for TestContext {
        fn drop(&mut self) {
            println!("Test teardown ...");
        }
    }

    fn setup() -> TestContext {
        let proof = read_and_decode_proof_file("src/instructions/test-data/op-proof-large.hex")
            .expect("could not read proof file");
        let event = read_and_decode_event_file("src/instructions/test-data/op-event-large.json")
            .expect("could not read event file");

        TestContext {
            proof,
            event,
            client_type: "proof_api".to_string(),
            signer: EthAddress::from_hex("8D3921B96A3815F403Fb3a4c7fF525969d16f9E0"),
            peptide_chain_id: 901,
        }
    }

    #[test]
    fn test_validate_proof_in_one_chunk() {
        let t = setup();

        let result = handler(
            &t.proof,
            &t.client_type,
            t.signer.as_bytes(),
            t.peptide_chain_id,
        );

        validate_result(t, result);
    }

    fn validate_result(t: TestContext, result: ValidateEventResult) {
        let (chain_id, event) = match result {
            ValidateEventResult::Valid(n, t) => (n, t),
            _ => panic!("expected valid proof"),
        };

        assert_eq!(84_532, chain_id);
        let mut topics: Vec<u8> = Vec::new();
        t.event
            .topics
            .iter()
            .for_each(|t| topics.extend(hex::decode(t.trim_start_matches("0x")).unwrap()));

        assert_eq!(topics, event.topics);
        assert_eq!(t.event.address, event.emitting_contract.to_hex());
        assert_eq!(
            hex::decode(t.event.data.trim_start_matches("0x")).unwrap(),
            event.unindexed_data
        );
    }

    #[test]
    #[ignore]
    fn test_invalid_recovered_signer_address() {}

    #[test]
    #[ignore]
    fn test_invalid_membership_proof() {}

    fn read_and_decode_proof_file(
        file_path: &str,
    ) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(file_path).expect("could not read hex file");
        let decoded = hex::decode(&contents.trim().as_bytes()[2..])?;
        Ok(decoded)
    }

    fn read_and_decode_event_file(
        file_path: &str,
    ) -> std::result::Result<Event, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(file_path).expect("could not read json file");
        let event: Event = serde_json::from_str(&contents).expect("error parsing JSON");
        Ok(event)
    }
}
