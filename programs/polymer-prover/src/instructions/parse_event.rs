use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Debug, PartialEq, Clone, Copy, Default, BorshSerialize, BorshDeserialize)]
pub struct EthAddress([u8; 20]);

impl EthAddress {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut array = [0u8; 20];
        array.copy_from_slice(bytes);
        EthAddress(array)
    }

    /// Converts a hex string (with or without "0x" prefix) into an EthAddress.
    pub fn from_hex(str: &str) -> Self {
        let mut addr = [0u8; 20];
        let _ = hex::decode_to_slice(str.trim_start_matches("0x"), &mut addr as &mut [u8]);
        EthAddress(addr)
    }

    /// Returns the address as a byte array.
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        "0x".to_owned() + &hex::encode(&self.0)
    }
}

#[derive(Debug, PartialEq, Clone, Default, BorshSerialize, BorshDeserialize)]
pub struct EthEvent {
    pub emitting_contract: EthAddress,
    pub topics: Vec<u8>,
    pub unindexed_data: Vec<u8>,
}

pub fn handler(raw_event: &[u8], num_topics: usize) -> EthEvent {
    // TODO check raw_event size

    let topics_end: usize = 32 * num_topics + 20;
    EthEvent {
        emitting_contract: EthAddress::from_bytes(&raw_event[..20]),
        topics: Vec::from(&raw_event[20..topics_end]),
        unindexed_data: Vec::from(&raw_event[topics_end..]),
    }
}

// TODO add test to verify the from_bytes() function behaves like go's common.BytesToAddress()
