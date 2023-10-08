#![no_std]
#![no_main]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::{format, net::TcpListener, println};

extern crate axstd as std;

use common::{bincode, new_payload, Payload, ARCEOS_PORT, MAGIC_HEADER};

use bincode::de::read::Reader;
use bincode::enc::write::Writer;

pub struct TcpIO<'a>(pub &'a mut TcpStream);

impl Reader for TcpIO<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), bincode::error::DecodeError> {
        self.0.read_exact(bytes).map_err(|err| {
            bincode::error::DecodeError::OtherString(format!("tcp recv error: {}", err))
        })?;
        Ok(())
    }
}

impl Writer for TcpIO<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.0.write_all(bytes).map_err(|err| {
            bincode::error::EncodeError::OtherString(format!("tcp send error: {}", err))
        })?;
        Ok(())
    }
}

#[no_mangle]
fn main() {
    let server = TcpListener::bind(("0.0.0.0", ARCEOS_PORT)).expect("failed to setup tcp server");
    loop {
        let (mut stream, addr) = server.accept().expect("failed to setup tcp connection");
        println!("received connection from {}", addr);
        let mut hdr_buf = [0u8; MAGIC_HEADER.len()];
        let ret = stream.read_exact(&mut hdr_buf);
        if let Err(err) = ret {
            println!("error: {}", err);
            continue;
        }
        if hdr_buf != MAGIC_HEADER {
            println!("connection from non-test client");
            continue;
        }

        let recv: Payload =
            bincode::decode_from_reader(TcpIO(&mut stream), bincode::config::standard())
                .expect("failed to decode payload");

        let default_payload = new_payload();
        assert_eq!(recv, default_payload);
        bincode::encode_into_writer(recv, TcpIO(&mut stream), bincode::config::standard())
            .expect("failed to send payload");
        stream.flush().expect("failed to flush");
        stream.shutdown().expect("failed to shutdown connection");
        break;
    }
}
