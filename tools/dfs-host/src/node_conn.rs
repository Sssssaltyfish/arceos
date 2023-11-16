use crossbeam::queue::SegQueue;
use std::net::TcpStream;
use std::sync::Arc;

use crate::host::RequestOnQueue;

pub struct DfsNodeConn {
    conn: TcpStream,
    message_queue: Arc<SegQueue<RequestOnQueue>>,
}

impl DfsNodeConn {
    pub fn new(conn: TcpStream, mq: Arc<SegQueue<RequestOnQueue>>) -> Self {
        DfsNodeConn {
            conn,
            message_queue: mq,
        }
    }

    pub fn handle_conn(&mut self) {}
}
