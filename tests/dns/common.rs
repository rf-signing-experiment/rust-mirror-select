#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rust_mirror_select::{Mirror, MirrorContents, MirrorSelectClient, MirrorSet};
use simple_dns::rdata::{RData, TXT};
use simple_dns::{CLASS, CharacterString, Packet, QTYPE, RCODE, ResourceRecord, TYPE};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

pub const MAX_CHARACTER_STRING: usize = 255;

type Zone = HashMap<String, Vec<String>>;

pub struct MockDnsServer {
    addr: SocketAddr,
    zone: Arc<Mutex<Zone>>,
    task: JoinHandle<()>,
}

impl MockDnsServer {
    pub async fn start() -> Self {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("bind mock DNS socket");
        let addr = socket.local_addr().expect("mock DNS local address");
        let zone = Arc::new(Mutex::new(Zone::new()));
        let task = tokio::spawn(serve(socket, Arc::clone(&zone)));
        Self { addr, zone, task }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn client(&self) -> MirrorSelectClient {
        MirrorSelectClient::with_timeout(self.addr, Duration::from_secs(2))
    }

    pub fn publish(&self, name: &str, set: &MirrorSet) {
        let entries = set
            .to_entries()
            .expect("encode mirrors")
            .into_iter()
            .map(|entry| entry.to_string());
        self.publish_raw(name, entries);
    }

    pub fn publish_raw<I, S>(&self, name: &str, entries: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let entries = entries.into_iter().map(Into::into).collect();
        self.zone.lock().unwrap().insert(normalize(name), entries);
    }

    pub fn remove(&self, name: &str) {
        self.zone.lock().unwrap().remove(&normalize(name));
    }
}

impl Drop for MockDnsServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn normalize(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

async fn serve(socket: UdpSocket, zone: Arc<Mutex<Zone>>) {
    let mut buf = vec![0; 512];
    loop {
        let Ok((len, peer)) = socket.recv_from(&mut buf).await else {
            return;
        };
        let Some(reply) = answer(&buf[..len], &zone.lock().unwrap()) else {
            continue;
        };
        let _ = socket.send_to(&reply, peer).await;
    }
}

fn answer(query: &[u8], zone: &Zone) -> Option<Vec<u8>> {
    let query = Packet::parse(query).ok()?;
    let question = query.questions.first()?.clone();

    let mut reply = Packet::new_reply(query.id());
    reply.questions.push(question.clone());

    match zone.get(&normalize(&question.qname.to_string())) {
        Some(entries) if question.qtype == QTYPE::TYPE(TYPE::TXT) => {
            for entry in entries {
                reply.answers.push(ResourceRecord::new(
                    question.qname.clone(),
                    CLASS::IN,
                    300,
                    RData::TXT(txt_record(entry)),
                ));
            }
        }
        Some(_) => {}
        None => *reply.rcode_mut() = RCODE::NameError,
    }

    reply.build_bytes_vec().ok()
}

fn txt_record(text: &str) -> TXT<'_> {
    text.as_bytes()
        .chunks(MAX_CHARACTER_STRING)
        .map(|chunk| CharacterString::new(chunk).expect("chunk fits a character string"))
        .fold(TXT::new(), TXT::with_char_string)
}

pub fn mirror(url: &str) -> Mirror {
    Mirror::new(
        url.parse().expect("valid mirror url"),
        MirrorContents::all(),
    )
}

pub fn mirrors<'a>(urls: impl IntoIterator<Item = &'a str>) -> MirrorSet {
    urls.into_iter().map(mirror).collect()
}
