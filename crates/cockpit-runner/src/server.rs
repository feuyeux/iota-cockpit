use std::io;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use crate::ipc::{
    RunnerHandler,
    proto::{IPC_VERSION, IpcError, RunnerRequest, RunnerResponse},
};

pub async fn serve(bind: &str, session_token: impl Into<String>) -> io::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    serve_listener(listener, session_token).await
}

pub async fn serve_listener(
    listener: TcpListener,
    session_token: impl Into<String>,
) -> io::Result<()> {
    let handler = Arc::new(Mutex::new(RunnerHandler::new(session_token)));
    loop {
        let (stream, _) = listener.accept().await?;
        let handler = Arc::clone(&handler);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, handler).await {
                eprintln!("cockpit-runner connection closed: {error}");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    handler: Arc<Mutex<RunnerHandler>>,
) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        let response = match serde_json::from_str::<RunnerRequest>(&line) {
            Ok(request) => handler.lock().await.dispatch(request),
            Err(error) => RunnerResponse {
                version: IPC_VERSION,
                correlation_id: "invalid-request".to_string(),
                ok: false,
                result: None,
                error: Some(IpcError {
                    code: "INVALID_REQUEST".to_string(),
                    message: error.to_string(),
                    details: None,
                    run_id: None,
                    tick: None,
                    correlation_id: "invalid-request".to_string(),
                }),
            },
        };
        let mut encoded =
            serde_json::to_vec(&response).map_err(|error| io::Error::other(error.to_string()))?;
        encoded.push(b'\n');
        write.write_all(&encoded).await?;
        write.flush().await?;
    }
    Ok(())
}
