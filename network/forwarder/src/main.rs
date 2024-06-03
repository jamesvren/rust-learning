#![allow(unused)]

use anyhow::Result;
use bytes::BytesMut;
use clap::Parser;
use log::{error, info, LevelFilter};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;
use std::os::unix::fs::PermissionsExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::task::JoinHandle;
use uuid::Uuid;

use forwarder::log as logging;

#[derive(Deserialize)]
struct Request {
    bash: String,
    owner: String,
}

#[derive(Debug, Serialize, Default)]
struct Response {
    output: String,
    error: String,
    code: Option<i32>,
}

async fn handle_stream(
    mut stream: impl AsyncReadExt + AsyncWriteExt + std::marker::Unpin,
) -> Result<()> {
    let mut data = BytesMut::with_capacity(4096);
    let n = stream.read_buf(&mut data).await?;
    let uuid = Uuid::new_v4();
    info!("[{uuid}] - Got Request({n} bytes): {data:?}");

    let mut response = Response {
        ..Default::default()
    };
    let request: Request = serde_json::from_slice::<Request>(&data)?;
    match run_cmd(&request.bash).await {
        Ok(output) => {
            response.output = String::from_utf8_lossy(output.stdout.as_slice()).to_string();
            response.error = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
            response.code = output.status.code();
        }
        Err(e) => {
            response.error = format!("{e}");
        }
    }
    info!("[{uuid}] - Got {response:?}");
    let response = serde_json::to_vec(&response)?;
    match stream.write_all(&response).await {
        Ok(_) => {
            info!(
                "[{uuid}] - Send response({} bytes) successfully.",
                response.len()
            )
        }
        Err(e) => {
            error!("[{uuid}] - Failed to send response: {e}");
        }
    }
    stream.shutdown().await?;
    Ok(())
}

async fn run_cmd(cmd: &str) -> Result<Output> {
    info!("Run command: {cmd}");
    Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stdin(Stdio::piped())
        .output()
        .await
        .map_err(anyhow::Error::from)
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Log rollback file size (Mb)
    #[arg(long, default_value_t = 50)]
    logsize: u64,
    /// Max rollback file number
    #[arg(long, default_value_t = 50)]
    logmax: u32,
    /// Log file path, the direcotry will be created automatically
    #[arg(long, default_value = "/var/log/forwarder/")]
    logdir: String,
    /// Enable TCP listener
    #[arg(long, group = "tcprpc")]
    tcp: bool,
    /// Tcp listen host
    #[arg(long, default_value = "127.0.0.1", requires = "tcprpc")]
    host: String,
    /// Tcp listen port
    #[arg(long, default_value_t = 7788, requires = "tcprpc")]
    port: u16,
}

const SOCK: &str = "/var/run/forwarder/forwarder.sock";
#[tokio::main]
async fn main() -> Result<()> {
    let opt = Cli::parse();
    if let Err(e) = logging::init(&opt.logdir, opt.logsize, opt.logmax) {
        println!("Failed to create logfile.");
        return Err(e);
    }

    let path = Path::new(SOCK);
    if path.exists() {
        fs::remove_file(SOCK)?;
    } else {
        let dir = path.parent().unwrap();
        if !dir.try_exists()? {
            fs::create_dir_all(dir)?;
        }
    }
    let mut handlers: Vec<JoinHandle<Result<()>>> = Vec::new();

    let unix_listener = UnixListener::bind(SOCK)?;
    let mut perms = fs::metadata(SOCK)?.permissions();
    perms.set_mode(0o666);
    fs::set_permissions(SOCK, perms)?;
    handlers.push(tokio::spawn(async move {
        loop {
            let (stream, addr) = unix_listener.accept().await?;
            info!("Got Request from {addr:?}");
            tokio::spawn(async move {
                match handle_stream(stream).await {
                    Err(e) => error!("{e}"),
                    _ => (),
                }
            });
        }
        Ok(())
    }));

    if opt.tcp {
        let host = format!("{}:{}", opt.host, opt.port);
        let tcp_listener = TcpListener::bind(&host).await?;

        handlers.push(tokio::spawn(async move {
            loop {
                let (stream, addr) = tcp_listener.accept().await?;
                info!("Got Request from {addr:?}");
                tokio::spawn(async move {
                    match handle_stream(stream).await {
                        Err(e) => error!("{e}"),
                        _ => (),
                    }
                });
            }
            Ok(())
        }));
    }

    for handle in handlers {
        handle.await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_cmd() {
        let output = run_cmd("echo test").await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "test\n"
        );
    }

    #[tokio::test]
    async fn test_run_cmd_pipe() {
        let output = run_cmd("echo test | grep test").await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "test\n"
        );
    }

    #[tokio::test]
    async fn test_run_cmd_glob() {
        let output = run_cmd("find . -name Car*").await.unwrap();
        assert_eq!(output.status.success(), false);
        println!("stderr: {}", unsafe {
            std::str::from_utf8_unchecked(&output.stderr)
        });

        let output = run_cmd("find . -name \"Car*\"").await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "./Cargo.toml\n./Cargo.lock\n"
        );
    }

    #[tokio::test]
    async fn test_run_cmd_bash() {
        let output = run_cmd("bash -c 'echo test'").await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "test\n"
        );
    }

    #[tokio::test]
    async fn test_run_cmd_exec1() {
        let output = run_cmd("echo `echo test`").await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "test\n"
        );
    }

    #[tokio::test]
    async fn test_run_cmd_exec2() {
        let output = run_cmd("echo $(echo test)").await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "test\n"
        );
    }

    #[tokio::test]
    async fn test_run_cmd_awk() {
        let output = run_cmd("echo hello world | awk '{print $1}'")
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "hello\n"
        );
    }
}
