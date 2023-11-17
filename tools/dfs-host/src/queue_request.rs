use core::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Mutex, thread};

use alloc::{sync::Arc, vec::Vec};
use axfs::distfs::request::Response;
use crossbeam::queue::SegQueue;

use crate::{host::NodeID, node_conn::PeerAction, utils::*};

pub struct MessageQueue(SegQueue<Arc<RequestFuture>>);

impl MessageQueue {
    pub fn new() -> Self {
        Self(SegQueue::new())
    }

    // call on producer
    pub fn submit_and_wait(&self, action: PeerAction) -> ReturnTypeYouNeed {
        let fut = Arc::new(RequestFuture::new(action));
        self.0.push(fut.clone());
        fut.wait_till_complete()
    }

    // call on consumer
    pub fn pop_to_work(&self, work: impl FnOnce(&PeerAction) -> ReturnTypeYouNeed) {
        let fut = self.wait_on_queue();
        let mut data = fut.waiter.lock().unwrap();
        let ret = work(&fut.action);
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

// directly forward serialized content to simplify everything
pub type SerializedResponseContent = Vec<u8>;
// give it a better name later
pub type ReturnTypeYouNeed = Response<SerializedResponseContent>;

pub struct RequestFuture {
    action: PeerAction,
    waiter: Mutex<Option<ReturnTypeYouNeed>>,
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

    fn wait_till_complete(&self) -> ReturnTypeYouNeed {
        while !self.done.load(core::sync::atomic::Ordering::Acquire) {
            thread::yield_now();
        }

        let mut data = self.waiter.lock().unwrap();
        data.take().unwrap()
    }
}
