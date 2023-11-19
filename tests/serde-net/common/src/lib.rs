#![no_std]
#![feature(variant_count)]

extern crate alloc;

use alloc::{borrow::ToOwned, string::String, vec, vec::Vec};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub use bincode;
pub use serde;

#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq)]
pub enum EnumPayload {
    A,
    B(String),
    C { field0: String, field1: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode, PartialEq, Eq)]
pub struct Payload {
    pub _u8: u8,
    pub _u64: u64,
    pub _usize: usize,
    pub _string: String,
    pub _vec_u8: Vec<u8>,
    pub _vec_string: Vec<String>,
    pub _enum: [EnumPayload; core::mem::variant_count::<EnumPayload>()],
}

pub fn new_payload() -> Payload {
    Payload {
        _u8: 42,
        _u64: 1234648,
        _usize: 20231008,
        _string: "Some rrrrrandom String".to_owned(),
        _vec_u8: vec![9, 8, 4, 2],
        _vec_string: vec!["Str1".to_owned(), "str2".to_owned()],
        _enum: [
            EnumPayload::B("Enum test".to_owned()),
            EnumPayload::A,
            EnumPayload::C {
                field0: "C Test".to_owned(),
                field1: 312312,
            },
        ],
    }
}

pub const ARCEOS_PORT: u16 = 5555;
pub const MAGIC_HEADER: &[u8] = b"ArceOS Test: NET";
