#[derive(Clone, Debug)]
pub enum TransportError {
    NotSupported,
}

/// Abstract transport
pub trait Transport: Send + Sync {
    /// get self node id
    fn nid(&self) -> u64;
    /// get total number of nodes
    fn num_nodes(&self) -> u64;
    /// get block by block id
    fn get(&self, nid: u64, bid: u64, buf: &mut [u8]) -> Result<usize, TransportError>;
    /// set block by block id
    fn set(&self, nid: u64, bid: u64, buf: &[u8]) -> Result<(), TransportError>;
    /// allocate an unused block id
    fn next_bid(&self) -> Option<u64>;

    /// mark a block as unused
    #[allow(unused)]
    fn discard(&self, nid: u64, bid: u64) -> Result<(), TransportError> {
        Err(TransportError::NotSupported)
    }
    /// compare-and-swap a block by block id,
    /// which allows easier implementation for high-performance data modification
    /// when concurrent write occurs
    #[allow(unused)]
    fn compare_and_swap(
        &self,
        nid: u64,
        bid: u64,
        old: &[u8],
        new: &[u8],
    ) -> Result<(), TransportError> {
        Err(TransportError::NotSupported)
    }
}
