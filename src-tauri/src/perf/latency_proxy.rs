use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Proxy TCP que atrasa cada pedaço de bytes em `one_way` por sentido (RTT =
/// 2 × `one_way`) e, opcionalmente, limita a banda. Basta para o que a Fase 0
/// mede — idas e voltas por comando e o peso de transferir o catálogo — sem
/// depender de uma ferramenta externa como o toxiproxy.
pub struct LatencyProxy {
    pub port: u16,
    task: JoinHandle<()>,
}

impl LatencyProxy {
    pub async fn start(upstream: SocketAddr, one_way: Duration, bytes_per_sec: Option<u64>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind do proxy");
        let port = listener.local_addr().expect("porta do proxy").port();

        let task = tokio::spawn(async move {
            while let Ok((client, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let Ok(server) = TcpStream::connect(upstream).await else {
                        return;
                    };
                    let _ = client.set_nodelay(true);
                    let _ = server.set_nodelay(true);

                    let (client_read, client_write) = client.into_split();
                    let (server_read, server_write) = server.into_split();

                    tokio::spawn(pipe(client_read, server_write, one_way, bytes_per_sec));
                    tokio::spawn(pipe(server_read, client_write, one_way, bytes_per_sec));
                });
            }
        });

        Self { port, task }
    }
}

impl Drop for LatencyProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Um sentido da conexão: a leitura carimba cada pedaço com o instante em que
/// ele "chega" do outro lado, e a escrita respeita esse instante e a banda —
/// assim a latência não serializa as leituras (como numa rede de verdade).
async fn pipe(
    mut reader: OwnedReadHalf,
    mut writer: OwnedWriteHalf,
    one_way: Duration,
    bytes_per_sec: Option<u64>,
) {
    let (tx, mut rx) = mpsc::unbounded_channel::<(Instant, Vec<u8>)>();

    let delivery = tokio::spawn(async move {
        let mut link_free_at = Instant::now();

        while let Some((due, chunk)) = rx.recv().await {
            tokio::time::sleep_until(due).await;

            if let Some(rate) = bytes_per_sec {
                let transmit = Duration::from_secs_f64(chunk.len() as f64 / rate as f64);
                link_free_at = link_free_at.max(Instant::now()) + transmit;
                tokio::time::sleep_until(link_free_at).await;
            }

            if writer.write_all(&chunk).await.is_err() {
                break;
            }
        }

        let _ = writer.shutdown().await;
    });

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tx.send((Instant::now() + one_way, buf[..n].to_vec())).is_err() {
                    break;
                }
            }
        }
    }

    drop(tx);
    let _ = delivery.await;
}
