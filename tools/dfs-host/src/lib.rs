#![cfg_attr(feature = "axstd", no_std)]

#[allow(unused_imports)]
#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

extern crate alloc;

pub mod host;

mod client_conn;
mod node_conn;
mod utils;
mod queue_request;