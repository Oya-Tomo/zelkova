use crate::types::*;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

pub struct RpcServer {
    listener: UnixListener,
    socket_path: std::path::PathBuf,
}

impl RpcServer {
    pub fn bind(socket_path: &Path) -> Result<Self> {
        if socket_path.exists() {
            std::fs::remove_file(socket_path).with_context(|| {
                format!(
                    "failed to remove existing socket at {}",
                    socket_path.display()
                )
            })?;
        }

        let listener = UnixListener::bind(socket_path)
            .with_context(|| format!("failed to bind socket at {}", socket_path.display()))?;

        Ok(Self {
            listener,
            socket_path: socket_path.to_path_buf(),
        })
    }

    pub fn accept_one(&self, handler: &dyn Fn(JsonRpcRequest) -> JsonRpcResponse) -> Result<()> {
        let (stream, _) = self
            .listener
            .accept()
            .context("failed to accept connection")?;
        handle_connection(stream, handler)?;
        Ok(())
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl Drop for RpcServer {
    fn drop(&mut self) {
        // Socket file may not exist if server failed during startup.
        let _result: std::io::Result<()> = std::fs::remove_file(&self.socket_path);
    }
}

fn handle_connection(
    stream: UnixStream,
    handler: &dyn Fn(JsonRpcRequest) -> JsonRpcResponse,
) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone().context("failed to clone stream")?);
    let mut writer = stream;

    let mut line = String::new();
    reader
        .read_line(&mut line)
        .context("failed to read request")?;

    if line.trim().is_empty() {
        return Ok(());
    }

    let request: JsonRpcRequest =
        serde_json::from_str(&line).context("failed to parse JSON-RPC request")?;
    let response = handler(request);
    let response_json = serde_json::to_string(&response).context("failed to serialize response")?;

    writer.write_all(response_json.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};
    use std::os::unix::net::UnixStream;
    use std::thread;

    fn test_socket() -> String {
        format!("/tmp/zelkova-test-{}.sock", std::process::id())
    }

    fn simple_handler(req: JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
            "echo" => JsonRpcResponse::success(req.id, serde_json::json!({"echo": req.params})),
            _ => JsonRpcResponse::error(
                req.id,
                JsonRpcError::not_found(format!("unknown method: {}", req.method)),
            ),
        }
    }

    #[test]
    fn server_accepts_and_responds() {
        let socket = test_socket();
        let server = RpcServer::bind(Path::new(&socket)).expect("test socket bind");

        let socket_clone = socket.clone();
        let handle = thread::spawn(move || {
            let mut stream = UnixStream::connect(&socket_clone).expect("connect to test socket");
            let request = JsonRpcRequest::new(1, "echo", Some(serde_json::json!("hello")));
            let json = serde_json::to_string(&request).expect("serialize request");
            stream.write_all(json.as_bytes()).expect("write request");
            stream.write_all(b"\n").expect("write newline");
            stream.flush().expect("flush stream");

            let mut reader = BufReader::new(stream);
            let mut response_line = String::new();
            reader.read_line(&mut response_line).expect("read response");
            let response: JsonRpcResponse =
                serde_json::from_str(&response_line).expect("parse response");
            assert!(response.error.is_none());
            assert!(response.result.is_some());
        });

        server
            .accept_one(&simple_handler)
            .expect("accept connection");
        handle.join().expect("test thread panicked");
    }

    #[test]
    fn server_unknown_method_returns_error() {
        let socket = format!("/tmp/zelkova-test-err-{}.sock", std::process::id());
        let server = RpcServer::bind(Path::new(&socket)).expect("test socket bind");

        let socket_clone = socket.clone();
        let handle = thread::spawn(move || {
            let mut stream = UnixStream::connect(&socket_clone).expect("connect to test socket");
            let request = JsonRpcRequest::new(2, "nonexistent", None);
            let json = serde_json::to_string(&request).expect("serialize request");
            stream.write_all(json.as_bytes()).expect("write request");
            stream.write_all(b"\n").expect("write newline");
            stream.flush().expect("flush stream");

            let mut reader = BufReader::new(stream);
            let mut response_line = String::new();
            reader.read_line(&mut response_line).expect("read response");
            let response: JsonRpcResponse =
                serde_json::from_str(&response_line).expect("parse response");
            assert!(response.error.is_some());
        });

        server
            .accept_one(&simple_handler)
            .expect("accept connection");
        handle.join().expect("test thread panicked");
    }
}
