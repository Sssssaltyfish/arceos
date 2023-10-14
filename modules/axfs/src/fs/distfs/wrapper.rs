use alloc::sync::{Arc, Weak};
use axerrno::{ax_err_type, AxResult};
use axfs_vfs::{
    TryFromPrimitive, VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodePerm, VfsNodeRef, VfsNodeType,
    VfsResult,
};
use axnet::TcpSocket;
use compact_str::{CompactString, ToCompactString};

use super::request::{self, recv_fsop, recv_fsop_serde, send_fsop, Action, NodeAttr, Request};

pub struct NodeWrapper {
    conn: Weak<TcpSocket>,
    relpath: CompactString,
}

impl NodeWrapper {
    pub fn new(conn: Weak<TcpSocket>, relpath: impl ToCompactString) -> Self {
        Self {
            conn,
            relpath: relpath.to_compact_string(),
        }
    }

    fn try_getconn(&self) -> VfsResult<Arc<TcpSocket>> {
        let ret = self.conn.upgrade().ok_or_else(|| {
            ax_err_type!(
                BadState,
                "trying to operate on filesystem that is already unmounted"
            )
        })?;
        Ok(ret)
    }

    fn with_path(&self, relpath: CompactString) -> Self {
        Self {
            conn: self.conn.clone(),
            relpath,
        }
    }
}

macro_rules! impl_vfs_op {
    ( $self:ident, $rety:ty, $action:ident ) => {{
        let this = $self;
        let conn = this.try_getconn()?;
        send_fsop(Request::new(&this.relpath, Action::$action), &conn)?;
        let ret: $rety = recv_fsop(&conn)?;
        VfsResult::<$rety>::Ok(ret)
    }};
    ( $self:ident, $rety:ty, $action:ident, $( $args:ident ),* ) => {{
        let this = $self;
        let conn = this.try_getconn()?;
        send_fsop(Request::new(&this.relpath, request::$action { $($args),* }.into()), &conn)?;
        let ret: $rety = recv_fsop(&conn)?;
        VfsResult::<$rety>::Ok(ret)
    }};
}

impl VfsNodeOps for NodeWrapper {
    fn open(&self) -> VfsResult {
        impl_vfs_op!(self, (), Open)
    }

    fn release(&self) -> VfsResult {
        impl_vfs_op!(self, (), Release)
    }

    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        let NodeAttr {
            mode,
            ty,
            size,
            blocks,
        } = impl_vfs_op!(self, NodeAttr, GetAttr)?;

        let mode = VfsNodePerm::from_bits(mode)
            .ok_or_else(|| ax_err_type!(InvalidData, format_args!("invalid mode: {:#o}", mode)))?;
        let ty = VfsNodeType::try_from_primitive(ty).map_err(|e| {
            ax_err_type!(InvalidData, format_args!("invalid type: {:#o}", e.number))
        })?;

        Ok(VfsNodeAttr::new(mode, ty, size, blocks))
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        let length = buf.len() as _;
        let conn = self.try_getconn()?;
        send_fsop(
            Request::new(&self.relpath, request::Read { length, offset }.into()),
            &conn,
        )?;
        let ret = conn.recv(buf)?;
        Ok(ret)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        let content = buf;
        impl_vfs_op!(self, usize, Write, offset, content)
    }

    fn fsync(&self) -> VfsResult {
        impl_vfs_op!(self, (), Fsync)
    }

    fn truncate(&self, size: u64) -> VfsResult {
        impl_vfs_op!(self, (), Trunc, size)
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        let relpath = || -> AxResult<_> {
            let conn = self.try_getconn()?;
            send_fsop(Request::new(&self.relpath, Action::GetParent), &conn)?;
            let ret = recv_fsop_serde(&conn)?;
            Ok(ret)
        }()
        .ok()?;
        Some(Arc::new(self.with_path(relpath)))
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let conn = self.try_getconn()?;
        send_fsop(Request::new(&self.relpath, request::Lookup { path }.into()), &conn)?;
        let relpath = recv_fsop_serde(&conn)?;

        Ok(Arc::new(self.with_path(relpath)))
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        let ty = ty as _;
        impl_vfs_op!(self, (), Create, path, ty)
    }

    fn remove(&self, path: &str) -> VfsResult {
        impl_vfs_op!(self, (), Remove, path)
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        unimplemented!();
    }

    fn rename(&self, src_path: &str, dst_path: &str) -> VfsResult {
        impl_vfs_op!(self, (), Rename, src_path, dst_path)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
