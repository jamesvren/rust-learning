#![allow(unused)]

use anyhow::Result;
use bytes::BytesMut;
use clap::Parser;
use log::{error, info, debug, LevelFilter};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;
use std::os::unix::fs::PermissionsExt;
use std::os::fd::AsRawFd;
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
    input: Option<String>,
    owner: String,
}

#[derive(Debug, Serialize, Default)]
struct Response {
    output: String,
    error: String,
    code: Option<i32>,
}

async fn handle_stream(
    mut stream: impl AsyncReadExt + AsyncWriteExt + std::marker::Unpin + AsRawFd,
    uuid: Uuid,
) -> Result<()> {
    const BUF_LEN: usize = 4096;
    let mut data = BytesMut::with_capacity(BUF_LEN);
    let mut n = stream.read_buf(&mut data).await?;
    while n % BUF_LEN == 0 {
        if n == 0 {
            break;
        }
        data.reserve(BUF_LEN);
        n = stream.read_buf(&mut data).await?;
    }
    n = data.len();
    let sock = stream.as_raw_fd();
    info!("[{uuid}][{sock}] - Got Request({n} bytes): {data:?}");

    let mut response = Response {
        ..Default::default()
    };
    let request: Request = serde_json::from_slice::<Request>(&data)?;
    match run_cmd(&request.bash, request.input.as_deref()).await {
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
    let sock = stream.as_raw_fd();
    match stream.write_all(&response).await {
        Ok(_) => {
            info!(
                "[{uuid}][{sock}] - Send response({} bytes) successfully.",
                response.len()
            )
        }
        Err(e) => {
            error!("[{uuid}][{sock}] - Failed to send response: {e}");
        }
    }
    stream.shutdown().await?;
    Ok(())
}

async fn run_cmd(cmd: &str, input: Option<&str>) -> Result<Output> {
    info!("Run command: {cmd}");
    let mut command = Command::new("bash");
    let command = command
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    match input {
        None => command.output().await.map_err(anyhow::Error::from),
        Some(input) => {
            debug!("Command Input: {input}");
            let mut child = command.spawn().map_err(anyhow::Error::from)?;
            match child.stdin {
                Some(ref mut stdin) => {
                    stdin.write_all(input.as_bytes()).await?;
                    child.wait_with_output().await.map_err(anyhow::Error::from)
                }
                None => Err(anyhow::anyhow!("STDIN not catched!!!")),
            }
        }
    }
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
            let uuid = Uuid::new_v4();
            info!("[{uuid}] Accept connection from {addr:?}");
            tokio::spawn(async move {
                match handle_stream(stream, uuid).await {
                    Err(e) => error!("[{uuid}] {e}"),
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
                let uuid = Uuid::new_v4();
                info!("[{uuid}] Accept connection from {addr:?}");
                tokio::spawn(async move {
                    match handle_stream(stream, uuid).await {
                        Err(e) => error!("[{uuid}] {e}"),
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
        let output = run_cmd("echo test", None).await.unwrap();
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
        let output = run_cmd("echo test | grep test", None).await.unwrap();
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
        let output = run_cmd("find . -name Car*", None).await.unwrap();
        assert_eq!(output.status.success(), false);
        println!("stderr: {}", unsafe {
            std::str::from_utf8_unchecked(&output.stderr)
        });

        let output = run_cmd("find . -name \"Car*\"", None).await.unwrap();
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
        let output = run_cmd("bash -c 'echo test'", None).await.unwrap();
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
        let output = run_cmd("echo `echo test`", None).await.unwrap();
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
        let output = run_cmd("echo $(echo test)", None).await.unwrap();
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
        let output = run_cmd("echo hello world | awk '{print $1}'", None)
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

    #[tokio::test]
    async fn test_run_cmd_stdin() {
        let output = run_cmd("cat", Some("ok"))
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr.len(), 0);
        assert_ne!(output.stdout.len(), 0);
        assert_eq!(
            unsafe { std::str::from_utf8_unchecked(&output.stdout) },
            "ok"
        );
    }
}
