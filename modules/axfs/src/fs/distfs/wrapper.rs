use alloc::sync::{Arc, Weak};
use axerrno::{ax_err, ax_err_type, AxError, AxResult};
use axfs_vfs::{
    VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodePerm, VfsNodeRef, VfsNodeType, VfsResult,
};
use compact_str::{CompactString, ToCompactString};

use super::{
    request::{
        self, recv_fsop, recv_fsop_serde, send_fsop, Action, NodeAttr, NodeTypeFromPrimitive,
        Request, Response,
    },
    SharedData,
};

pub struct NodeWrapper {
    data: Weak<SharedData>,
    relpath: CompactString,
}

impl NodeWrapper {
    pub fn new(data: Weak<SharedData>, relpath: impl ToCompactString) -> Self {
        Self {
            data,
            relpath: relpath.to_compact_string(),
        }
    }

    fn try_get_data(&self) -> VfsResult<Arc<SharedData>> {
        log::debug!("distfs: try_get_data");
        let ret = self.data.upgrade().ok_or_else(|| {
            ax_err_type!(
                BadState,
                "trying to operate on filesystem that is already unmounted"
            )
        })?;
        Ok(ret)
    }

    fn with_path(&self, relpath: CompactString) -> Self {
        log::debug!("distfs: with_path");
        Self {
            data: self.data.clone(),
            relpath,
        }
    }
}

macro_rules! impl_vfs_op {
    ( $self:ident, $rety:ty, $action:ident ) => {{
        let this = $self;
        let data = this.try_get_data()?;
        let conn = &data.conn;
        send_fsop(Request::new(&this.relpath, Action::$action), &conn)?;
        let ret: $rety = recv_fsop(&conn)?;
        ret
    }};
    ( $self:ident, $rety:ty, $action:ident, $( $args:ident ),* ) => {{
        let this = $self;
        let data = this.try_get_data()?;
        let conn = &data.conn;
        send_fsop(Request::new(&this.relpath, request::$action { $($args),* }.into()), &conn)?;
        let ret: $rety = recv_fsop(&conn)?;
        ret
    }};
}

impl VfsNodeOps for NodeWrapper {
    fn open(&self) -> VfsResult {
        log::debug!("distfs: open");
        impl_vfs_op!(self, Response<()>, Open).map_code()
    }

    fn release(&self) -> VfsResult {
        log::debug!("distfs: release");
        impl_vfs_op!(self, Response<()>, Release).map_code()
    }

    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        log::debug!("distfs: get_attr");
        let NodeAttr {
            mode,
            ty,
            size,
            blocks,
        } = impl_vfs_op!(self, Response<NodeAttr>, GetAttr).map_code()?;

        let mode = VfsNodePerm::from_bits(mode)
            .ok_or_else(|| ax_err_type!(InvalidData, format_args!("invalid mode: {:#o}", mode)))?;
        let ty = ty.to_node_type()?;

        Ok(VfsNodeAttr::new(mode, ty, size, blocks))
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        log::debug!("distfs: read_at");
        let length = buf.len() as _;
        let data = self.try_get_data()?;
        let conn = &data.conn;
        send_fsop(
            Request::new(&self.relpath, request::Read { length, offset }.into()),
            &conn,
        )?;
        let stat: Response<u64> = recv_fsop(&conn)?;

        // If error code is returned, early exit;
        // otherwise recv content of file.
        let size = stat.map_code()? as usize;
        if size == 0 {
            return Ok(0);
        }
        let ret = conn.recv(buf)?;
        match size.cmp(&ret) {
            core::cmp::Ordering::Less => panic!(
                "recved {} bytes while expecting {} bytes, which implies severe logical bug",
                ret, size
            ),
            core::cmp::Ordering::Equal => Ok(size),
            core::cmp::Ordering::Greater => ax_err!(
                UnexpectedEof,
                format_args!("recved {} bytes while expecting {} bytes", ret, size)
            ),
        }
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        log::debug!("distfs: write_at");
        let content = buf;
        let ret = impl_vfs_op!(self, Response<u64>, Write, offset, content).map_code()?;
        Ok(ret as _)
    }

    fn fsync(&self) -> VfsResult {
        log::debug!("distfs: fsync");
        impl_vfs_op!(self, Response<()>, Fsync).map_code()
    }

    fn truncate(&self, size: u64) -> VfsResult {
        log::debug!("distfs: truncate");
        impl_vfs_op!(self, Response<()>, Trunc, size).map_code()
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        log::debug!("distfs: parent");
        if self.relpath == "./" {
            let data = self.try_get_data().ok()?;
            let ret = data.parent.wait();
            return Some(ret.clone());
        }

        // To support possible links
        let relpath = || -> AxResult<_> {
            let data = self.try_get_data()?;
            let conn = &data.conn;
            send_fsop(Request::new(&self.relpath, Action::GetParent), &conn)?;
            let ret: Response<_> = recv_fsop_serde(&conn)?;
            let ret = ret.map_code()?;
            Ok(ret)
        }()
        .ok()?;
        Some(Arc::new(self.with_path(relpath)))
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        log::debug!("distfs: lookup");
        let path = path.trim_start_matches('/');
        let data = self.try_get_data()?;
        let conn = &data.conn;
        send_fsop(
            Request::new(&self.relpath, request::Lookup { path }.into()),
            &conn,
        )?;
        log::debug!("distfs: lookup after send");
        let relpath: Response<_> = recv_fsop_serde(&conn)?;
        let relpath = relpath.map_code()?;

        Ok(Arc::new(self.with_path(relpath)))
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        log::debug!("distfs: create");
        let ty = ty as _;
        let path = path.trim_start_matches('/');
        impl_vfs_op!(self, Response<()>, Create, path, ty).map_code()
    }

    fn remove(&self, path: &str) -> VfsResult {
        log::debug!("distfs: remove");
        let path = path.trim_start_matches('/');
        impl_vfs_op!(self, Response<()>, Remove, path).map_code()
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        log::debug!("distfs: read_dir");
        let size = dirents.len() as _;
        let start_idx = start_idx as _;
        let data = self.try_get_data()?;
        let conn = &data.conn;

        send_fsop(
            Request::new(&self.relpath, request::ReadDir { size, start_idx }.into()),
            conn,
        )?;
        let actual_len: u64 = recv_fsop(conn)?;
        let actual_len = actual_len as _;

        for ent in dirents.iter_mut().take(actual_len) {
            let entry: request::DirEntry = recv_fsop_serde(conn)?;
            *ent = entry.try_into()?;
        }

        Ok(actual_len)
    }

    fn rename(&self, src_path: &str, dst_path: &str) -> VfsResult {
        log::debug!("distfs: rename");
        impl_vfs_op!(self, Response<()>, Rename, src_path, dst_path).map_code()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        log::debug!("distfs: as_any");
        self
    }
}

trait MapCode<T> {
    fn map_code(self) -> VfsResult<T>;
}

impl<T> MapCode<T> for request::Response<T> {
    fn map_code(self) -> VfsResult<T> {
        self.map_err(|code| {
            AxError::try_from(code).unwrap_or_else(|e| {
                ax_err_type!(InvalidData, format_args!("invalid error code: {}", e))
            })
        })
    }
}
