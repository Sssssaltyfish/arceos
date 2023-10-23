use core::fmt::Debug;

use alloc::format;
use axnet::TcpSocket;
use bincode::de::read::Reader;
use bincode::enc::write::Writer;

pub const BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();

pub(super) struct TcpIO<'a>(pub &'a TcpSocket);

impl Reader for TcpIO<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), bincode::error::DecodeError> {
        self.0.recv(bytes).map_err(|err| {
            bincode::error::DecodeError::OtherString(format!("tcp recv error: {}", err))
        })?;
        Ok(())
    }
}

impl Writer for TcpIO<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.0.send(bytes).map_err(|err| {
            bincode::error::EncodeError::OtherString(format!("tcp send error: {}", err))
        })?;
        Ok(())
    }
}

impl Debug for TcpIO<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TcpIO")
            .field("local_addr", &self.0.local_addr())
            .field("peer_addr", &self.0.peer_addr())
            .field("non_blocking", &self.0.is_nonblocking())
            .finish_non_exhaustive()
    }
}
