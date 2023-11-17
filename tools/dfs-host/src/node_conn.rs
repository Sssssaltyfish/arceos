use crossbeam::queue::SegQueue;
use std::net::TcpStream;
use std::sync::Arc;

use crate::utils::PathBuf;
use crate::queue_request::RequestOnQueue;

pub struct DfsNodeOutConn {
    conn: TcpStream,
    message_queue: Arc<SegQueue<RequestOnQueue>>,
}

pub struct DfsNodeInConn {
    root_path: PathBuf,
    conn: TcpStream,
}

impl DfsNodeOutConn {
    pub fn new(conn: TcpStream, mq: Arc<SegQueue<RequestOnQueue>>) -> Self {
        DfsNodeOutConn {
            conn,
            message_queue: mq,
        }
    }

    pub fn handle_conn(&mut self) {}
}

impl DfsNodeInConn {
    pub fn new(root_path: PathBuf, conn: TcpStream) -> Self {
        DfsNodeInConn { root_path, conn }
    }

    pub fn handle_conn(&mut self) {}
}
