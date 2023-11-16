use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use alloc::{format, string::String, vec::Vec};

use dashmap::DashMap;
use crossbeam::queue::{ArrayQueue, SegQueue};

use crate::utils::PathBuf;
use crate::client_conn::DfsClientConn;
use crate::node_conn::DfsNodeConn;

#[cfg(feature = "axstd")]
use crate::utils::*;

pub type NodeID = u32;
pub type RequestOnQueue = u32;

const NODE_START_PORT: NodeID = 8000;
const CLIENT_START_PORT: NodeID = 9000;
const START_ADDRESS: &str = "127.0.0.1";

pub struct DfsHost {
    node_id: NodeID,
    root_path: PathBuf,
    peers_worker: Arc<DashMap<NodeID, Arc<SegQueue<RequestOnQueue>>>>,
    file_index: Arc<DashMap<String, NodeID>>,
}

impl DfsHost {
    pub fn new(node_id: NodeID, root_path: PathBuf) -> Self {
        DfsHost {
            node_id,
            root_path,
            peers_worker: Arc::new(DashMap::new()),
            file_index: Arc::new(DashMap::new()),
        }
    }

    pub fn start_listening(&mut self) {
        // Bind a TcpListener to listen on bind_address

        // bind tcp conn with other nodes
        for node in 0..self.node_id {
            let conn = TcpStream::connect(&format!("{}:{}", START_ADDRESS, NODE_START_PORT + node))
                .expect(&format!("Failed to connect to node {}", node));
            let mq = Arc::new(SegQueue::new());
            let mut peer_conn = DfsNodeConn::new(conn, mq.clone());
            self.peers_worker.insert(node, mq.clone());
            // Distribute to handle thread
            thread::spawn({
                move || {
                    peer_conn.handle_conn();
                }
            });
        }
        // ask for file tree
        if self.node_id != 0 {
            // get file tree from root node
        }

        // listen to other connections
        let peer_listener = TcpListener::bind(&format!(
            "{}:{}",
            START_ADDRESS,
            NODE_START_PORT + self.node_id
        ))
        .unwrap();
        logger::info!(
            "Listening for incoming peer connections on {}...",
            self.node_id + NODE_START_PORT
        );
        let clients_listener = TcpListener::bind(&format!(
            "{}:{}",
            START_ADDRESS,
            CLIENT_START_PORT + self.node_id
        ))
        .unwrap();
        logger::info!(
            "Listening for incoming client connections on {}...",
            self.node_id + CLIENT_START_PORT
        );

        let peers_worker_ref = self.peers_worker.clone();
        let peer_thread = thread::spawn(move || {
            for stream in peer_listener.incoming() {
                match stream {
                    Ok(peer_stream) => {
                        logger::info!(
                            "Accepted a new peer connection from: {:?}",
                            peer_stream.peer_addr()
                        );
                        let mq = Arc::new(SegQueue::new());
                        let mut peer_conn = DfsNodeConn::new(peer_stream, mq.clone());
                        let p = &peers_worker_ref;
                        let node_id = p.len();
                        p.insert(node_id as NodeID, mq.clone());
                        // Distribute to handle thread
                        thread::spawn({
                            move || {
                                peer_conn.handle_conn();
                            }
                        });
                    }
                    Err(e) => {
                        logger::error!("Error accepting connection: {}", e);
                    }
                }
            }
        });

        for stream in clients_listener.incoming() {
            match stream {
                Ok(client_stream) => {
                    logger::info!(
                        "Accepted a new client connection from: {:?}",
                        client_stream.peer_addr()
                    );
                    // Create a new DfsClientConn instance for each connection and store it
                    let mut new_client = DfsClientConn::new(
                        // Initialize fields for DfsClientConn as needed
                        self.root_path.clone(),
                        client_stream,
                        self.peers_worker.clone(),
                        self.file_index.clone(),
                    );
                    // Distribute to handle thread
                    thread::spawn({
                        move || {
                            new_client.handle_conn();
                        }
                    });
                }
                Err(e) => {
                    logger::error!("Error accepting connection: {}", e);
                }
            }
        }
    }
}
