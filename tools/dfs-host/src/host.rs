use std::collections::BTreeMap;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::client_conn::DfsClientConn;
use crate::node_conn::DfsNodeConn;
pub type NodeID = u32;
const NODE_START_PORT: NodeID = 8000;
const CLIENT_START_PORT: NodeID = 9000;
const START_ADDRESS: &str = "127.0.0.1";

pub struct DfsHost {
    node_id: NodeID,
    root_path: PathBuf,
    clients: Arc<Mutex<Vec<Arc<Mutex<DfsClientConn>>>>>,
    peers: Arc<Mutex<BTreeMap<NodeID, Arc<Mutex<DfsNodeConn>>>>>,
    file_index: Arc<Mutex<BTreeMap<String, NodeID>>>,
}

impl DfsHost {
    pub fn new(node_id: NodeID, root_path: PathBuf) -> Self {
        DfsHost {
            node_id,
            root_path,
            clients: Arc::new(Mutex::new(Vec::new())),
            peers: Arc::new(Mutex::new(BTreeMap::new())),
            file_index: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn start_listening(&mut self) {
        // Bind a TcpListener to listen on bind_address

        // bind tcp conn with other nodes
        for node in 0..self.node_id {
            let conn = TcpStream::connect(&format!("{}:{}", START_ADDRESS, NODE_START_PORT + node))
                .expect(&format!("Failed to connect to node {}", node));
            let peer_conn = Arc::new(Mutex::new(DfsNodeConn::new(conn)));
            self.peers.lock().unwrap().insert(node, peer_conn.clone());
            // Distribute to handle thread
            thread::spawn({
                move || {
                    let mut peer_thread = peer_conn.lock().unwrap();
                    peer_thread.handle_conn();
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
        println!(
            "Listening for incoming peer connections on {}...",
            self.node_id + NODE_START_PORT
        );
        let clients_listener = TcpListener::bind(&format!(
            "{}:{}",
            START_ADDRESS,
            CLIENT_START_PORT + self.node_id
        ))
        .unwrap();
        println!(
            "Listening for incoming client connections on {}...",
            self.node_id + CLIENT_START_PORT
        );

        let peers_ref = self.peers.clone();
        let peer_thread = thread::spawn(move || {
            for stream in peer_listener.incoming() {
                match stream {
                    Ok(peer_stream) => {
                        println!(
                            "Accepted a new peer connection from: {:?}",
                            peer_stream.peer_addr()
                        );
                        let new_peer = Arc::new(Mutex::new(DfsNodeConn::new(
                            peer_stream,
                        )));
                        let mut p = peers_ref.lock().unwrap();
                        let node_id = p.len();
                        p.insert(node_id as NodeID, new_peer.clone());
                        // Distribute to handle thread
                        thread::spawn({
                            move || {
                                let mut peer_thread = new_peer.lock().unwrap();
                                peer_thread.handle_conn();
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                    }
                }
            }
        });

        let client_root_ref = self.root_path.clone();
        let clients_ref = self.clients.clone();
        let peers_ref = self.peers.clone();
        let file_index_ref = self.file_index.clone();
        let client_thread = thread::spawn(move || {
            for stream in clients_listener.incoming() {
                match stream {
                    Ok(client_stream) => {
                        println!(
                            "Accepted a new client connection from: {:?}",
                            client_stream.peer_addr()
                        );
                        // Create a new DfsClientConn instance for each connection and store it
                        let new_client = Arc::new(Mutex::new(DfsClientConn::new(
                            // Initialize fields for DfsClientConn as needed
                            client_root_ref.clone(),
                            client_stream,
                            peers_ref.clone(),
                            file_index_ref.clone(),
                        )));
                        let mut c = clients_ref.lock().unwrap();
                        c.push(new_client.clone());
                        // Distribute to handle thread
                        thread::spawn({
                            move || {
                                let mut client_thread = new_client.lock().unwrap();
                                client_thread.handle_conn();
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                    }
                }
            }
        });

        peer_thread.join().expect("Error in peer thread");
        client_thread.join().expect("Error in client thread");
    }
}
