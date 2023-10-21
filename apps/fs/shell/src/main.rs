#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

macro_rules! path_to_str {
    ($path:expr) => {{
        #[cfg(not(feature = "axstd"))]
        {
            $path.to_str().unwrap() // Path/OsString -> &str
        }
        #[cfg(feature = "axstd")]
        {
            $path.as_str() // String -> &str
        }
    }};
}

mod cmd;

#[cfg(feature = "use-ramfs")]
mod ramfs;

use std::{
    fs::{File, OpenOptions},
    io::{self, prelude::*, BufReader, Stdin, Stdout},
};

use noline::builder::EditorBuilder;

const MAX_CMD_LEN: usize = 256;

struct Stdio<'a>(&'a mut Stdin, &'a mut Stdout);

impl noline::sync::Read for Stdio<'_> {
    type Error = std::io::Error;

    fn read(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0u8];
        self.0.read_exact(&mut buf)?;
        Ok(buf[0])
    }
}

impl noline::sync::Write for Stdio<'_> {
    type Error = std::io::Error;

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.1.write_all(buf)?;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.1.flush()?;
        Ok(())
    }
}

fn prompt_history() -> std::io::Result<File> {
    const HISTORY_PATH: &str = "/shell.history";
    macro_rules! is_not_found {
        ( $e:expr ) => {{
            #[cfg(feature = "axstd")]
            {
                matches!($e, io::Error::NotFound)
            }
            #[cfg(not(feature = "axstd"))]
            {
                matches!($e.kind(), io::ErrorKind::NotFound)
            }
        }};
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(HISTORY_PATH)
        .or_else(|e| {
            if is_not_found!(e) {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(HISTORY_PATH)
            } else {
                Err(e)
            }
        })
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    let mut io = Stdio(&mut stdin, &mut stdout);

    let mut editor = EditorBuilder::new_static::<MAX_CMD_LEN>()
        .with_static_history::<25>()
        .build_sync(&mut io)
        .unwrap();

    let history = BufReader::new(prompt_history().unwrap())
        .lines()
        .filter_map(|s| s.ok());
    editor.load_history(history);

    cmd::run_cmd("help".as_bytes());

    loop {
        let prompt = format!(
            "arceos:{}$ ",
            path_to_str!(std::env::current_dir().unwrap())
        );
        if let Ok(line) = editor.readline(&prompt, &mut io) {
            cmd::run_cmd(line.as_bytes());
        } else {
            break;
        }
    }

    let mut history = prompt_history().unwrap();
    for slice in editor.get_history() {
        for (_, &c) in slice {
            history.write_all(&[c]).unwrap();
        }
        history.write_all(&[b'\n']).unwrap();
    }
}
