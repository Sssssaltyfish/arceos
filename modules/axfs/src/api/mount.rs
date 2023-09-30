use axerrno::ax_err;
use axio as io;

bitflags::bitflags! {
    pub struct MountFlag: u32 {
        /// Mount read-only.
        const RDONLY = 1;
        /// Ignore suid and sgid bits.
        const NOSUID = 2;
        /// Disallow access to device special files.
        const NODEV = 4;
        /// Disallow program execution.
        const NOEXEC = 8;
        /// Writes are synced at once.
        const SYNCHRONOUS = 16;
        /// Alter flags of a mounted FS.
        const REMOUNT = 32;
        /// Allow mandatory locks on an FS.
        const MANDLOCK = 64;
        /// Directory modifications are synchronous.
        const DIRSYNC = 128;
        /// Do not follow symlinks.
        const NOSYMFOLLOW = 256;
        /// Do not update access times.
        const NOATIME = 1024;
        /// Do not update directory access times.
        const NODIRATIME = 2048;
        /// Bind directory at different place.
        const BIND = 4096;
        /// Move directory to different place.
        const MOVE = 8192;
        const REC = 16384;
        const SILENT = 32768;
        /// VFS does not apply the umask.
        const POSIXACL = 1 << 16;
        /// Change to unbindable.
        const UNBINDABLE = 1 << 17;
        /// Change to private.
        const PRIVATE = 1 << 18;
        /// Change to slave.
        const SLAVE = 1 << 19;
        /// Change to shared.
        const SHARED = 1 << 20;
        /// Update atime relative to mtime/ctime.
        const RELATIME = 1 << 21;
        /// This is a kern_mount call.
        const KERNMOUNT = 1 << 22;
        /// Update inode I_version field.
        const I_VERSION = 1 << 23;
        /// Always perform atime updates.
        const STRICTATIME = 1 << 24;
        /// Update the on-disk [acm]times lazily.
        const LAZYTIME = 1 << 25;
        const ACTIVE = 1 << 30;
        const NOUSER = 1 << 31;
    }
}

impl Default for MountFlag {
    fn default() -> Self {
        Self::empty()
    }
}

#[allow(unused)]
pub fn mount(
    source: &str,
    target: &str,
    ty: &str,
    flag: MountFlag,
    data: Option<&[u8]>,
) -> io::Result<()> {
    if !flag.is_empty() {
        return ax_err!(Unsupported);
    }
    crate::root::mount;
    unimplemented!()
}

pub fn umount(path: &str) -> io::Result<()> {
    crate::root::umount(path)
}
