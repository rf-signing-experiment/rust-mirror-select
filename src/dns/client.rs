use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use simple_dns::rdata::RData;
use simple_dns::{CLASS, Name, Packet, PacketFlag, QCLASS, QTYPE, Question, RCODE, TYPE};
use tokio::net::UdpSocket;

use crate::Error;
use crate::consts::{DEFAULT_TIMEOUT, MAX_RESPONSE_LEN};

#[derive(Debug)]
pub struct MirrorSelectClient {
    server: SocketAddr,
    timeout: Duration,
    next_id: AtomicU16,
}

impl MirrorSelectClient {
    pub fn new(server: SocketAddr) -> Self {
        Self::with_timeout(server, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(server: SocketAddr, timeout: Duration) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos() as u16);
        Self {
            server,
            timeout,
            next_id: AtomicU16::new(seed),
        }
    }

    pub async fn lookup_txt(&self, name: &str) -> Result<Vec<String>, Error> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut query = Packet::new_query(id);
        query.set_flags(PacketFlag::RECURSION_DESIRED);
        query.questions.push(Question::new(
            Name::new(name)?,
            QTYPE::TYPE(TYPE::TXT),
            QCLASS::CLASS(CLASS::IN),
            false,
        ));

        let local: SocketAddr = if self.server.is_ipv4() {
            (Ipv4Addr::UNSPECIFIED, 0).into()
        } else {
            (Ipv6Addr::UNSPECIFIED, 0).into()
        };
        let socket = UdpSocket::bind(local).await?;
        socket.connect(self.server).await?;
        socket.send(&query.build_bytes_vec()?).await?;

        let mut buf = vec![0; MAX_RESPONSE_LEN];
        let len = tokio::time::timeout(self.timeout, async {
            loop {
                let len = socket.recv(&mut buf).await?;
                let is_reply = Packet::parse(&buf[..len]).is_ok_and(|packet| {
                    packet.id() == id && packet.has_flags(PacketFlag::RESPONSE)
                });
                if is_reply {
                    return Ok::<_, Error>(len);
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout {
            server: self.server,
            timeout: self.timeout,
        })??;

        let response = Packet::parse(&buf[..len])?;
        match response.rcode() {
            RCODE::NoError => response
                .answers
                .into_iter()
                .filter_map(|record| match record.rdata {
                    RData::TXT(txt) => Some(String::try_from(txt).map_err(Error::from)),
                    _ => None,
                })
                .collect(),
            rcode => Err(Error::Rcode {
                name: name.to_owned(),
                rcode,
            }),
        }
    }
}
