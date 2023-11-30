use alloc::boxed::Box;
use axnet::TcpSocket;
use dashmap::DashMap;
use driver_block::BlockDriverOps;
use driver_common::BaseDriverOps;
use spin::RwLock;

use crate::transport::{Transport, TransportError};

const NOT_ALLOCATED: u64 = u64::MAX;

#[derive(Debug)]
pub struct PeerNode {
    pub(crate) nid: u64,
    pub(crate) block_num: u64,
    pub(crate) conn: TcpSocket,
}

pub struct DistBlockDevice {
    nid: u64,
    block_size: u64,
    inner: RwLock<Box<dyn BlockDriverOps>>,
    allocated_bid: RwLock<Box<[u64]>>,
    peers: DashMap<u64, PeerNode>,
}

impl DistBlockDevice {
    pub fn new(inner: Box<dyn BlockDriverOps>) -> Self {
        unimplemented!()
    }

    pub fn test(&self) {
        self.allocated_bid.read()[0];
    }

    fn map_block_id(&self, block_id: u64) -> (u64, u64) {
        unimplemented!()
    }
}

impl Transport for DistBlockDevice {
    fn nid(&self) -> u64 {
        self.nid
    }

    fn num_nodes(&self) -> u64 {
        self.peers.len() as _
    }

    fn get(&self, nid: u64, bid: u64, buf: &mut [u8]) -> Result<usize, TransportError> {
        let inner = self.inner.upgradeable_read();
        if buf.len() > inner.block_size() {
            return Err(todo!());
        }

        if nid == self.nid {
            let blk_idx = self.allocated_bid.read()[bid as usize];
            if blk_idx == NOT_ALLOCATED {
                return Err(todo!());
            }

            inner.upgrade().read_block(blk_idx, buf).map_err(|_| todo!())?;
            return Ok(buf.len());
        }

        todo!()
    }

    fn set(&self, nid: u64, bid: u64, buf: &[u8]) -> Result<(), TransportError> {
        let inner = self.inner.upgradeable_read();
        if buf.len() > inner.block_size() {
            return Err(todo!());
        }

        if nid == self.nid {
            let blk_idx = self.allocated_bid.read()[bid as usize];
            if blk_idx == NOT_ALLOCATED {
                return Err(todo!());
            }

            inner.upgrade().write_block(blk_idx, buf).map_err(|_| todo!())?;
            return Ok(());
        }

        todo!()
    }

    fn next_bid(&self) -> Option<u64> {
        let idx = self
            .allocated_bid
            .read()
            .iter()
            .enumerate()
            .find_map(|(bid, &blk_idx)| {
                if blk_idx == NOT_ALLOCATED {
                    Some(bid as _)
                } else {
                    None
                }
            });
        idx
    }
}

impl BaseDriverOps for DistBlockDevice {
    fn device_name(&self) -> &str {
        "dist_block"
    }

    fn device_type(&self) -> driver_common::DeviceType {
        driver_common::DeviceType::Block
    }
}

impl BlockDriverOps for DistBlockDevice {
    fn num_blocks(&self) -> u64 {
        let peer_num_blocks: u64 = self.peers.iter().map(|peer| peer.block_num).sum();
        self.inner.read().num_blocks() + peer_num_blocks
    }

    fn block_size(&self) -> usize {
        self.block_size as _
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> driver_block::DevResult {
        let (nid, bid) = self.map_block_id(block_id);
        let _ = self.get(nid, bid, buf).map_err(|err| unimplemented!())?;
        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> driver_block::DevResult {
        let (nid, bid) = self.map_block_id(block_id);
        let _ = self.set(nid, bid, buf).map_err(|err| unimplemented!())?;
        Ok(())
    }

    fn flush(&mut self) -> driver_block::DevResult {
        Ok(())
    }
}
