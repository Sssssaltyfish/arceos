#![cfg_attr(feature = "axstd", no_std)]

#[cfg(feature = "axstd")]
extern crate axstd as std;

extern crate alloc;

mod client_conn;
mod conn_utils;
mod host;
mod node_conn;
mod queue_request;
mod utils;

#[cfg(not(feature = "axstd"))]
fn main() {
    use host::DfsHost;
    use std::env;

    env_logger::builder()
        .filter_level(logger::LevelFilter::Debug)
        .init();
    let args: Vec<String> = env::args().collect();
    let node_id: u32 = args[1].parse().unwrap();
    let mut host = DfsHost::new(node_id, env::current_dir().unwrap());
    let _ = host.start_listening();
}

#[cfg(feature = "axstd")]
#[no_mangle]
fn main() {
    unimplemented!("dfs-host is not ready to be a arceos app")
}
