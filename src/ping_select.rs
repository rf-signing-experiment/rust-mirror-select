use std::time::Instant;

use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::consts::PING_TIMEOUT;
use crate::{Error, Mirror, MirrorSet, SelectStrategy};

#[derive(Clone, Copy, Debug, Default)]
pub struct PingSelect;

impl SelectStrategy for PingSelect {
    async fn get_best<'a>(&mut self, mirrors: &'a MirrorSet) -> Result<&'a Mirror, Error> {
        let mut pings = JoinSet::new();
        for (index, mirror) in mirrors.mirrors().iter().enumerate() {
            let url = mirror.url();
            let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
                continue;
            };
            let target = (host.to_owned(), port);
            pings.spawn(async move {
                let start = Instant::now();
                timeout(PING_TIMEOUT, TcpStream::connect(target))
                    .await
                    .ok()?
                    .ok()?;
                Some((start.elapsed(), index))
            });
        }

        let mut responses = Vec::new();
        while let Some(joined) = pings.join_next().await {
            if let Ok(Some(response)) = joined {
                responses.push(response);
            }
        }
        responses
            .into_iter()
            .min()
            .map(|(_, index)| &mirrors.mirrors()[index])
            .ok_or(Error::NoReachableMirror)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use tokio::net::{TcpListener, TcpSocket};

    use super::*;
    use crate::MirrorContents;

    async fn listener() -> (TcpListener, Mirror) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/").parse().unwrap();
        (listener, Mirror::new(url, MirrorContents::all()))
    }

    fn closed() -> (TcpSocket, Mirror) {
        let socket = TcpSocket::new_v4().unwrap();
        socket.bind((Ipv4Addr::LOCALHOST, 0).into()).unwrap();
        let port = socket.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/").parse().unwrap();
        (socket, Mirror::new(url, MirrorContents::all()))
    }

    #[tokio::test]
    async fn picks_the_mirror_that_answers() {
        let (_listener, reachable) = listener().await;
        let (_a, closed_a) = closed();
        let (_b, closed_b) = closed();
        let set: MirrorSet = [closed_a, reachable.clone(), closed_b]
            .into_iter()
            .collect();

        let best = PingSelect.get_best(&set).await.unwrap();
        assert_eq!(best, &reachable);
    }

    #[tokio::test]
    async fn skips_mirrors_without_a_host() {
        let (_listener, reachable) = listener().await;
        let local = Mirror::new("file:///srv/mirror".parse().unwrap(), MirrorContents::all());
        let set: MirrorSet = [local, reachable.clone()].into_iter().collect();

        assert_eq!(PingSelect.get_best(&set).await.unwrap(), &reachable);
    }

    #[tokio::test]
    async fn fails_when_nothing_answers() {
        let (_a, closed_a) = closed();
        let (_b, closed_b) = closed();
        let set: MirrorSet = [closed_a, closed_b].into_iter().collect();
        assert!(matches!(
            PingSelect.get_best(&set).await,
            Err(Error::NoReachableMirror)
        ));
    }

    #[tokio::test]
    async fn fails_on_empty_set() {
        assert!(matches!(
            PingSelect.get_best(&MirrorSet::default()).await,
            Err(Error::NoReachableMirror)
        ));
    }
}
