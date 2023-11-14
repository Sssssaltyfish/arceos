use std::net::TcpStream;

pub struct DfsNodeConn {
    conn: TcpStream,
}

impl DfsNodeConn {
    pub fn new(conn: TcpStream) -> Self {
        DfsNodeConn { conn }
    }

    pub fn handle_conn(&mut self) {}
}
