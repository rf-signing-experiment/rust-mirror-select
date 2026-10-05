#![allow(dead_code)]

use std::future::pending;
use std::net::Ipv4Addr;
use std::time::Duration;

use rust_mirror_select::{Mirror, MirrorContents, MirrorSet};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::sleep;

pub const BODY_LEN: usize = 10 * 1024 * 1024;
pub const CHUNK_LEN: usize = 64 * 1024;
pub const FASTEST: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    Respond(Duration),
    Silent,
    Interrupt,
    Refuse,
    Infinite,
    Undersized,
}

pub fn graded() -> [Behavior; 5] {
    [300, 100, 0, 200, 400].map(|ms| Behavior::Respond(Duration::from_millis(ms)))
}

pub fn rotations() -> impl Iterator<Item = [Behavior; 5]> {
    (0..5).map(|shift| {
        let mut behaviors = graded();
        behaviors.rotate_left(shift);
        behaviors
    })
}

pub fn fastest_index(behaviors: &[Behavior]) -> usize {
    behaviors
        .iter()
        .enumerate()
        .filter_map(|(index, behavior)| match behavior {
            Behavior::Respond(delay) => Some((*delay, index)),
            _ => None,
        })
        .min()
        .map(|(_, index)| index)
        .unwrap()
}

pub struct MockMirror {
    pub mirror: Mirror,
    pub behavior: Behavior,
    task: Option<JoinHandle<()>>,
    reserved: Option<TcpSocket>,
}

impl MockMirror {
    pub async fn start(behavior: Behavior) -> Self {
        let (port, task, reserved) = match behavior {
            Behavior::Refuse => {
                let socket = TcpSocket::new_v4().unwrap();
                socket.bind((Ipv4Addr::LOCALHOST, 0).into()).unwrap();
                (socket.local_addr().unwrap().port(), None, Some(socket))
            }
            behavior => {
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
                let port = listener.local_addr().unwrap().port();
                (port, Some(tokio::spawn(serve(listener, behavior))), None)
            }
        };
        let url = format!("http://127.0.0.1:{port}/").parse().unwrap();
        Self {
            mirror: Mirror::new(url, MirrorContents::all()),
            behavior,
            task,
            reserved,
        }
    }
}

impl Drop for MockMirror {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

pub struct Farm {
    pub servers: Vec<MockMirror>,
}

impl Farm {
    pub async fn start(behaviors: impl IntoIterator<Item = Behavior>) -> Self {
        let mut servers = Vec::new();
        for behavior in behaviors {
            servers.push(MockMirror::start(behavior).await);
        }
        Self { servers }
    }

    pub fn set(&self) -> MirrorSet {
        self.servers.iter().map(|s| s.mirror.clone()).collect()
    }

    pub fn mirror(&self, index: usize) -> &Mirror {
        &self.servers[index].mirror
    }

    pub fn responders(&self) -> impl Iterator<Item = &Mirror> {
        self.servers
            .iter()
            .filter(|s| s.behavior != Behavior::Refuse)
            .map(|s| &s.mirror)
    }
}

async fn serve(listener: TcpListener, behavior: Behavior) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(handle(stream, behavior));
    }
}

async fn handle(mut stream: TcpStream, behavior: Behavior) {
    let mut request = [0; 4096];
    let mut len = 0;
    while !request[..len].ends_with(b"\r\n\r\n") && len < request.len() {
        match stream.read(&mut request[len..]).await {
            Ok(0) | Err(_) => return,
            Ok(n) => len += n,
        }
    }
    let head = |len: usize| {
        format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n")
    };
    let body = vec![0; BODY_LEN];
    match behavior {
        Behavior::Respond(delay) => {
            sleep(delay).await;
            let _ = stream.write_all(head(BODY_LEN).as_bytes()).await;
            let _ = stream.write_all(&body).await;
        }
        Behavior::Interrupt => {
            let _ = stream.write_all(head(BODY_LEN).as_bytes()).await;
            let _ = stream.write_all(&body[..BODY_LEN / 2]).await;
        }
        Behavior::Undersized => {
            let _ = stream.write_all(head(CHUNK_LEN).as_bytes()).await;
            let _ = stream.write_all(&body[..CHUNK_LEN]).await;
        }
        Behavior::Infinite => {
            let chunked =
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
            if stream.write_all(chunked.as_bytes()).await.is_err() {
                return;
            }
            let chunk = format!("{CHUNK_LEN:x}\r\n");
            while stream.write_all(chunk.as_bytes()).await.is_ok()
                && stream.write_all(&body[..CHUNK_LEN]).await.is_ok()
                && stream.write_all(b"\r\n").await.is_ok()
            {}
        }
        Behavior::Silent => pending().await,
        Behavior::Refuse => {}
    }
}
