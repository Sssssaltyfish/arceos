use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use alloc::{format, string::String};

use dashmap::DashMap;

use crate::client_conn::DfsClientConn;
use crate::node_conn::{DfsNodeInConn, DfsNodeOutConn};
use crate::queue_request::MessageQueue;
use crate::utils::PathBuf;

#[cfg(feature = "axstd")]
use crate::utils::*;

pub type NodeID = u32;

const NODE_START_PORT: NodeID = 8000;
const CLIENT_START_PORT: NodeID = 9000;
const START_ADDRESS: &str = "127.0.0.1";

pub struct DfsHost {
    node_id: NodeID,
    root_path: PathBuf,
    peers_worker: Arc<DashMap<NodeID, Arc<MessageQueue>>>,
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

        let node_id_ref = self.node_id;
        let root_path_ref = self.root_path.clone();
        let peers_worker_ref = self.peers_worker.clone();
        let file_index_ref = self.file_index.clone();
        let _ = thread::spawn(move || {
            let mut in_conn_count = 0;
            for stream in peer_listener.incoming() {
                match stream {
                    Ok(in_stream) => {
                        logger::info!(
                            "Accepted a new peer connection from: {:?}",
                            in_stream.peer_addr()
                        );
                        let mut in_conn = DfsNodeInConn::new(
                            node_id_ref,
                            root_path_ref.clone(),
                            in_stream,
                            peers_worker_ref.clone(),
                            file_index_ref.clone(),
                        );
                        in_conn_count += 1;
                        // Distribute to handle thread
                        thread::spawn({
                            move || {
                                in_conn.handle_conn();
                            }
                        });
                        if (&peers_worker_ref).len() < in_conn_count {
                            let out_stream = TcpStream::connect(&format!(
                                "{}:{}",
                                START_ADDRESS,
                                NODE_START_PORT + in_conn_count as u32
                            ))
                            .expect(&format!("Failed to connect to node {}", in_conn_count));
                            let p = &peers_worker_ref;
                            let node_id = p.len();
                            let init_file_index_ref = file_index_ref.clone();
                            let mq = Arc::new(MessageQueue::new());
                            p.insert(node_id as NodeID, mq.clone());
                            let mut out_conn = DfsNodeOutConn::new(out_stream, mq.clone());
                            thread::spawn({
                                move || {
                                    if node_id_ref == 0 {
                                        out_conn.send_init_index(init_file_index_ref.clone());
                                    }
                                    out_conn.handle_conn();
                                }
                            });
                        }
                    }
                    Err(e) => {
                        logger::error!("Error accepting connection: {}", e);
                    }
                }
            }
        });

        // bind tcp conn with other nodes
        let peers_worker_ref = self.peers_worker.clone();
        for node in 0..self.node_id {
            let out_stream =
                TcpStream::connect(&format!("{}:{}", START_ADDRESS, NODE_START_PORT + node))
                    .expect(&format!("Failed to connect to node {}", node));
            let p = &peers_worker_ref;
            let node_id = p.len();
            let mq = Arc::new(MessageQueue::new());
            p.insert(node_id as NodeID, mq.clone());
            let mut out_conn = DfsNodeOutConn::new(out_stream, mq.clone());
            thread::spawn({
                move || {
                    out_conn.handle_conn();
                }
            });
        }
        // ask for file tree
        if self.node_id != 0 {
            // get file tree from root node
        }

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
                        self.node_id,
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
