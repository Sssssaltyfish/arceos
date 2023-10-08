#![allow(non_upper_case_globals)]
#![cfg(test)]

use std::{
    fs::File,
    io::{self, ErrorKind, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process,
    time::{Duration, Instant},
};

use anyhow::{bail, Error};
use common::{new_payload, Payload, ARCEOS_PORT, MAGIC_HEADER};
use lazy_static::lazy_static;
use log::*;

trait PathToStr {
    fn path_to_str(&self) -> anyhow::Result<String>;
}

impl PathToStr for Path {
    fn path_to_str(&self) -> anyhow::Result<String> {
        self.to_path_buf()
            .into_os_string()
            .into_string()
            .map_err(|_| Error::msg("invalid path"))
    }
}

lazy_static! {
    static ref root: PathBuf = Path::new("../../../").canonicalize().unwrap();
    static ref test_dir: PathBuf = root
        .join(file!())
        .parent()
        .unwrap()
        .join("../../")
        .canonicalize()
        .unwrap();
    static ref arceos_path: PathBuf = test_dir.join("./arceos");
}

fn test_arch(arch: &str, timeout: Duration) -> anyhow::Result<()> {
    let record_stdout = File::create(arceos_path.join(format!("arceos_{}.stdout.log", arch)))?;
    let record_stderr = File::create(arceos_path.join(format!("arceos_{}.stderr.log", arch)))?;

    info!("Building arceos app");
    let mut arceos = process::Command::new("make")
        .args([
            "-C".to_owned(),
            root.path_to_str()?,
            "run".to_owned(),
            format!("APP={}", arceos_path.path_to_str()?),
            "NET=y".to_owned(),
            format!("ARCH={}", arch),
        ])
        .stdout(record_stdout)
        .stderr(record_stderr)
        .spawn()?;

    info!("Trying to connect to arceos app");
    let start = Instant::now();
    let mut conn = loop {
        if start.elapsed() > timeout {
            arceos.kill()?;
            bail!(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection to qemu timeout"
            ));
        }
        let ret = TcpStream::connect(("127.0.0.1", ARCEOS_PORT));
        match ret {
            Ok(conn) => break conn,
            Err(e) if e.kind() == ErrorKind::ConnectionRefused => continue,
            e @ _ => e?,
        };
    };

    info!("Communicating with arceos app");
    conn.write_all(MAGIC_HEADER)?;
    let _encode =
        bincode::encode_into_std_write(new_payload(), &mut conn, bincode::config::standard())?;
    let decode: Payload = bincode::decode_from_std_read(&mut conn, bincode::config::standard())?;
    assert_eq!(decode, new_payload());
    assert!(arceos.wait()?.success());
    Ok(())
}

#[test]
fn test_all_archs() -> anyhow::Result<()> {
    let _ = env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .try_init();

    let archs = ["x86_64", "aarch64", "riscv64"];

    for arch in archs {
        info!("Testing {}", arch);
        let ret = test_arch(arch, Duration::from_secs(10));
        if let Err(e) = ret {
            error!("Error while testing {}: {}", arch, e);
        } else {
            info!("Test of {} done!", arch);
        }
    }

    Ok(())
}
