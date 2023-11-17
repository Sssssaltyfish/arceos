use std::io::{Read, Write};
use std::net::TcpStream;

#[cfg(not(feature = "axstd"))]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use alloc::{
    format,
    string::{String, ToString},
};

use axerrno::AxError;
use axfs::distfs::request::{Request, Response};
use axfs::distfs::BINCODE_CONFIG;

use bincode::enc::write::Writer;
use bincode::Encode;

#[cfg(feature = "axstd")]
use crate::utils::*;

use crate::utils::io_err_to_axerr;


pub(super) struct Tcpio<'a>(pub &'a mut TcpStream);

impl Writer for Tcpio<'_> {
    fn write(&mut self, bytes: &[u8]) -> core::result::Result<(), bincode::error::EncodeError> {
        self.0
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::OtherString(e.to_string()))
    }
}

pub fn read_data_from_conn(buff: &mut [u8], conn: &mut TcpStream) -> usize {
    let bytes_read = conn
        .read(buff)
        .map_err(|e| {
            logger::error!("Error reading bytes from connection: {:?}", conn);
            send_err_to_conn(conn, io_err_to_axerr(e))
        })
        .unwrap();
    bytes_read
}

pub fn deserialize_client_request_from_buff<'a>(
    buff: &'a [u8],
    bytes_read: usize,
) -> axfs::distfs::request::Request<'a, 'a> {
    let (req, _) =
        bincode::borrow_decode_from_slice::<Request, _>(&buff[..bytes_read], BINCODE_CONFIG)
            .map_err(|e| {
                logger::error!("Error deserializing from connection: {}", e);
            })
            .unwrap();
    req
}

pub fn send_ok_to_conn<T: Encode>(conn: &mut TcpStream, con: T) {
    let res = Response::Ok(con);
    bincode::encode_into_writer(&res, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back success response from connection: {:?}",
        conn
    ))
}

pub fn send_err_to_conn(conn: &mut TcpStream, e: AxError) {
    let res_err: Response<String> = Response::Err(e.code());
    bincode::encode_into_writer(&res_err, Tcpio(conn), BINCODE_CONFIG).expect(&format!(
        "Error happen when writing back error response from connection: {:?}",
        conn
    ));
}
