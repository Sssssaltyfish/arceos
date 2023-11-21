use core::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Mutex, thread};

use alloc::{string::String, sync::Arc, vec::Vec};
use axfs::distfs::request::Response;
use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::{host::NodeID, utils::*};

pub struct MessageQueue(SegQueue<Arc<RequestFuture>>);

impl MessageQueue {
    pub fn new() -> Self {
        Self(SegQueue::new())
    }

    // call on producer
    pub fn submit_and_wait(&self, action: PeerAction) -> ResponseFromPeer {
        let fut = Arc::new(RequestFuture::new(action));
        self.0.push(fut.clone());
        fut.wait_till_complete()
    }

    // call on consumer
    pub fn pop_to_work(&self, work: impl FnOnce(&PeerAction) -> ResponseFromPeer) {
        let fut = self.wait_on_queue();
        logger::debug!("Recieved new peer request to send: {:?}", fut.action);
        let mut data = fut.waiter.lock().unwrap();
        let ret = work(&fut.action);
        logger::debug!("Recieved response for peer request {:?}", ret.clone());
        *data = Some(ret);

        std::sync::atomic::compiler_fence(Ordering::Acquire);
        fut.done.store(true, Ordering::Release);
    }

    fn wait_on_queue(&self) -> Arc<RequestFuture> {
        loop {
            match self.0.pop() {
                Some(fut) => break fut,
                None => thread::yield_now(),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerAction {
    SerializedAction(Vec<u8>),
    InitIndex(DashMap<String, NodeID>),
    InsertIndex(DashMap<String, NodeID>),
    RemoveIndex(Vec<String>),
    UpdateIndex(DashMap<String, String>),
}

// directly forward serialized content to simplify everything
pub type SerializedResponseContent = Vec<u8>;
// give it a better name later
pub type ResponseFromPeer = Vec<SerializedResponseContent>;

pub struct RequestFuture {
    action: PeerAction,
    waiter: Mutex<Option<ResponseFromPeer>>,
    done: AtomicBool,
}

impl RequestFuture {
    pub fn new(action: PeerAction) -> Self {
        Self {
            action,
            waiter: Mutex::new(None),
            done: AtomicBool::new(false),
        }
    }

    fn wait_till_complete(&self) -> ResponseFromPeer {
        while !self.done.load(core::sync::atomic::Ordering::Acquire) {
            thread::yield_now();
        }

        let mut data = self.waiter.lock().unwrap();
        data.take().unwrap()
    }
}
