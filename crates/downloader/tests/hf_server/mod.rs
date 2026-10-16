#![allow(dead_code)]

use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tiny_http::{Header, Request, Response, Server, StatusCode};

const CDN_PREFIX: &str = "/cdn/";
const RESOLVE_MARKER: &str = "/resolve/";
const LINK_LIFETIME: Duration = Duration::from_secs(3600);
pub(crate) const CDN_ETAG: &str = "\"cdn-object-1\"";

#[derive(Clone, Debug)]
pub(crate) struct ServerBehaviour {
    pub(crate) content: Vec<u8>,
    pub(crate) revision: String,
    pub(crate) linked_etag: String,
    pub(crate) expired_links: u32,
    pub(crate) ignore_ranges: bool,
    pub(crate) chunk: usize,
    pub(crate) chunk_delay: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecordedRequest {
    pub(crate) path: String,
    pub(crate) range: Option<String>,
    pub(crate) if_range: Option<String>,
    pub(crate) status: u16,
}

pub(crate) struct HfServer {
    server: Arc<Server>,
    base: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    listener: Option<JoinHandle<()>>,
}

impl HfServer {
    pub(crate) fn start(behaviour: ServerBehaviour) -> Self {
        let server = Arc::new(Server::http("127.0.0.1:0").expect("the local server binds"));
        let address = server
            .server_addr()
            .to_ip()
            .expect("the server listens on an IP address");
        let base = format!("http://{address}");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let listener = {
            let server = Arc::clone(&server);
            let requests = Arc::clone(&requests);
            let base = base.clone();
            let issued = Arc::new(Mutex::new(0_u32));
            thread::spawn(move || {
                for request in server.incoming_requests() {
                    let behaviour = behaviour.clone();
                    let requests = Arc::clone(&requests);
                    let issued = Arc::clone(&issued);
                    let base = base.clone();
                    thread::spawn(move || answer(request, &behaviour, &base, &issued, &requests));
                }
            })
        };
        Self {
            server,
            base,
            requests,
            listener: Some(listener),
        }
    }

    pub(crate) fn base(&self) -> &str {
        &self.base
    }

    pub(crate) fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("the request log is never poisoned").clone()
    }

    pub(crate) fn resolves(&self) -> usize {
        self.requests()
            .iter()
            .filter(|request| request.path.contains(RESOLVE_MARKER))
            .count()
    }

    pub(crate) fn transfers(&self) -> Vec<RecordedRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.path.starts_with(CDN_PREFIX))
            .collect()
    }
}

impl Drop for HfServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(listener) = self.listener.take() {
            listener.join().expect("the listener thread finishes");
        }
    }
}

fn answer(
    request: Request,
    behaviour: &ServerBehaviour,
    base: &str,
    issued: &Mutex<u32>,
    requests: &Mutex<Vec<RecordedRequest>>,
) {
    let path = request.url().split('?').next().unwrap_or_default().to_owned();
    let range = header_value(&request, "Range");
    let if_range = header_value(&request, "If-Range");
    let record = |status: u16| {
        requests
            .lock()
            .expect("the request log is never poisoned")
            .push(RecordedRequest {
                path: path.clone(),
                range: range.clone(),
                if_range: if_range.clone(),
                status,
            });
    };
    if path.contains(RESOLVE_MARKER) {
        let link = {
            let mut issued = issued.lock().expect("the link counter is never poisoned");
            *issued += 1;
            *issued
        };
        let file = path.rsplit('/').next().unwrap_or_default();
        let expires = (SystemTime::now() + LINK_LIFETIME)
            .duration_since(UNIX_EPOCH)
            .expect("the clock is after the epoch")
            .as_secs();
        let location = format!("{base}{CDN_PREFIX}{link}/{file}?Expires={expires}");
        record(302);
        let headers = vec![
            header("Location", &location),
            header("X-Repo-Commit", &behaviour.revision),
            header("X-Linked-Etag", &format!("\"{}\"", behaviour.linked_etag)),
            header("X-Linked-Size", &behaviour.content.len().to_string()),
        ];
        let _ = request.respond(Response::new(StatusCode(302), headers, io::empty(), Some(0), None));
        return;
    }
    let link: u32 = path
        .trim_start_matches(CDN_PREFIX)
        .split('/')
        .next()
        .and_then(|number| number.parse().ok())
        .unwrap_or(0);
    if link <= behaviour.expired_links {
        record(403);
        let _ = request.respond(Response::new(StatusCode(403), Vec::new(), io::empty(), Some(0), None));
        return;
    }
    let total = behaviour.content.len();
    let start = range
        .as_deref()
        .and_then(|range| range.strip_prefix("bytes="))
        .and_then(|range| range.strip_suffix('-'))
        .and_then(|start| start.parse::<usize>().ok())
        .filter(|start| *start < total);
    let honours_range = !behaviour.ignore_ranges && if_range.as_deref().is_none_or(|validator| validator == CDN_ETAG);
    let mut headers = vec![header("ETag", CDN_ETAG), header("Accept-Ranges", "bytes")];
    let (status, offset) = match start {
        Some(start) if honours_range => {
            headers.push(header("Content-Range", &format!("bytes {start}-{}/{total}", total - 1)));
            (206, start)
        }
        _ => (200, 0),
    };
    record(status);
    let body = Throttled {
        content: behaviour.content[offset..].to_vec(),
        position: 0,
        chunk: behaviour.chunk,
        delay: behaviour.chunk_delay,
    };
    let _ = request.respond(Response::new(
        StatusCode(status),
        headers,
        body,
        Some(total - offset),
        None,
    ));
}

fn header_value(request: &Request, name: &'static str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str().to_owned())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("test headers are valid")
}

struct Throttled {
    content: Vec<u8>,
    position: usize,
    chunk: usize,
    delay: Duration,
}

impl Read for Throttled {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.content.len() {
            return Ok(0);
        }
        if self.position > 0 {
            thread::sleep(self.delay);
        }
        let length = buffer.len().min(self.chunk).min(self.content.len() - self.position);
        buffer[..length].copy_from_slice(&self.content[self.position..self.position + length]);
        self.position += length;
        Ok(length)
    }
}
