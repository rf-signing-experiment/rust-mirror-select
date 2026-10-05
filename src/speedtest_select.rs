use std::time::{Duration, Instant};

use reqwest::Client;

use crate::consts::{SPEEDTEST_PATH, SPEEDTEST_SIZE, SPEEDTEST_TIMEOUT};
use crate::{Error, Mirror, MirrorSet, SelectStrategy};

#[derive(Clone, Copy, Debug)]
pub struct SpeedtestSelect {
    timeout: Duration,
}

impl Default for SpeedtestSelect {
    fn default() -> Self {
        Self::with_timeout(SPEEDTEST_TIMEOUT)
    }
}

impl SpeedtestSelect {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }

    async fn bytes_per_second(client: &Client, mirror: &Mirror) -> Option<f64> {
        let url = mirror.url().join(SPEEDTEST_PATH).ok()?;
        let start = Instant::now();
        let mut response = client.get(url).send().await.ok()?.error_for_status().ok()?;
        if response.content_length() != Some(SPEEDTEST_SIZE) {
            return None;
        }
        let mut bytes = 0u64;
        while let Some(chunk) = response.chunk().await.ok()? {
            bytes += chunk.len() as u64;
            if bytes > SPEEDTEST_SIZE {
                return None;
            }
        }
        let seconds = start.elapsed().as_secs_f64();
        (bytes == SPEEDTEST_SIZE).then(|| bytes as f64 / seconds)
    }
}

impl SelectStrategy for SpeedtestSelect {
    async fn get_best<'a>(&mut self, mirrors: &'a MirrorSet) -> Result<&'a Mirror, Error> {
        let client = Client::builder().timeout(self.timeout).no_gzip().build()?;
        let mut best: Option<(f64, &Mirror)> = None;
        for mirror in mirrors.mirrors() {
            if let Some(rate) = Self::bytes_per_second(&client, mirror).await
                && best.is_none_or(|(fastest, _)| rate > fastest)
            {
                best = Some((rate, mirror));
            }
        }
        best.map(|(_, mirror)| mirror)
            .ok_or(Error::NoReachableMirror)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpSocket};
    use tokio::time::sleep;

    use super::*;
    use crate::MirrorContents;

    const BODY_LEN: usize = SPEEDTEST_SIZE as usize;

    async fn serve(status: &'static str, delay: Duration) -> Mirror {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut byte = [0];
                    while !request.ends_with(b"\r\n\r\n")
                        && stream.read(&mut byte).await.unwrap_or(0) == 1
                    {
                        request.push(byte[0]);
                    }
                    sleep(delay).await;
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {BODY_LEN}\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(&vec![0; BODY_LEN]).await;
                });
            }
        });
        let url = format!("http://127.0.0.1:{port}/").parse().unwrap();
        Mirror::new(url, MirrorContents::all())
    }

    #[tokio::test]
    async fn picks_the_fastest_download() {
        let slow = serve("200 OK", Duration::from_millis(400)).await;
        let fast = serve("200 OK", Duration::ZERO).await;
        let missing = serve("404 Not Found", Duration::ZERO).await;
        let set: MirrorSet = [slow, missing, fast.clone()].into_iter().collect();

        assert_eq!(
            SpeedtestSelect::default().get_best(&set).await.unwrap(),
            &fast
        );
    }

    #[tokio::test]
    async fn fails_when_nothing_serves_the_file() {
        let socket = TcpSocket::new_v4().unwrap();
        socket.bind((Ipv4Addr::LOCALHOST, 0).into()).unwrap();
        let port = socket.local_addr().unwrap().port();
        let refused = Mirror::new(
            format!("http://127.0.0.1:{port}/").parse().unwrap(),
            MirrorContents::all(),
        );
        let missing = serve("404 Not Found", Duration::ZERO).await;
        let set: MirrorSet = [refused, missing].into_iter().collect();

        assert!(matches!(
            SpeedtestSelect::default().get_best(&set).await,
            Err(Error::NoReachableMirror)
        ));
    }

    #[tokio::test]
    async fn fails_on_empty_set() {
        assert!(matches!(
            SpeedtestSelect::default()
                .get_best(&MirrorSet::default())
                .await,
            Err(Error::NoReachableMirror)
        ));
    }
}
