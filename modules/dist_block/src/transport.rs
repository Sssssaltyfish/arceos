#[derive(Clone, Debug)]
pub struct TransportError {}

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
    fn next(&self) -> u64;
}
