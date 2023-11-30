use alloc::boxed::Box;
use driver_block::BlockDriverOps;

const NOT_ALLOCATED: u64 = u64::MAX;

pub struct DistBlockDevice {
    nid: u64,
    inner: Box<dyn BlockDriverOps>,
}

impl DistBlockDevice {
    pub fn new(inner: Box<dyn BlockDriverOps>) -> Self {
        unimplemented!()
    }

    pub fn test(&self) {
        
    }
}
