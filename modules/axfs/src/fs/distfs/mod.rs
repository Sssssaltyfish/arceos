use alloc::sync::Arc;
use axerrno::AxResult;
use axfs_vfs::{VfsNodeRef, VfsOps, VfsResult};
use axnet::TcpSocket;
use spin::Once;

use self::wrapper::NodeWrapper;

mod tcpio;

pub mod request;
pub mod wrapper;

pub use tcpio::BINCODE_CONFIG;

struct SharedData {
    conn: TcpSocket,
    parent: Once<VfsNodeRef>,
}

/// A distributed filesystem that implements [`VfsOps`].
pub struct DistFileSystem {
    data: Arc<SharedData>,
    root: Arc<NodeWrapper>,
}

impl DistFileSystem {
    /// Construct a new [`DistFileSystem`].
    ///
    /// `conn` must be a connected [`TcpSocket`].
    pub fn new(conn: TcpSocket) -> Self {
        let data = Arc::new(SharedData {
            conn,
            parent: Once::new(),
        });
        let root = Arc::new(NodeWrapper::new(Arc::downgrade(&data), "./"));
        Self { data, root }
    }

    pub fn disconnect(&self) -> AxResult {
        self.data.conn.shutdown()?;
        Ok(())
    }
}

impl VfsOps for DistFileSystem {
    fn root_dir(&self) -> VfsNodeRef {
        self.root.clone()
    }

    fn mount(&self, _path: &str, mount_point: VfsNodeRef) -> VfsResult {
        self.data.parent.call_once(|| mount_point.clone());
        Ok(())
    }

    fn umount(&self) -> VfsResult {
        self.disconnect()?;
        Ok(())
    }
}
