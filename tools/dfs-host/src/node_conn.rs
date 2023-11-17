use std::net::TcpStream;
use std::sync::Arc;

use alloc::string::String;
use alloc::vec::Vec;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::host::NodeID;
use crate::queue_request::MessageQueue;
use crate::utils::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIndex {
    pub index: DashMap<String, NodeID>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerAction {
    SerializedAction(Vec<u8>),
    UpdateIndex(UpdateIndex),
}

pub struct DfsNodeOutConn {
    conn: TcpStream,
    message_queue: Arc<MessageQueue>,
}

pub struct DfsNodeInConn {
    root_path: PathBuf,
    conn: TcpStream,
}

impl DfsNodeOutConn {
    pub fn new(conn: TcpStream, mq: Arc<MessageQueue>) -> Self {
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
