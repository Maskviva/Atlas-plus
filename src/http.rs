use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use levilamina::Logger;

pub struct HttpServer {
    running: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
    workers: Vec<JoinHandle<()>>,
    addr: String,
}

impl HttpServer {
    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub fn shutdown(mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
    }
}

pub fn serve(
    bind: &str,
    port: u16,
    root: PathBuf,
    workers: usize,
    logger: Logger,
) -> std::io::Result<HttpServer> {
    let listener = TcpListener::bind((bind, port))?;
    listener.set_nonblocking(true)?;
    let addr = format!("{}:{}", bind, port);

    let running = Arc::new(AtomicBool::new(true));

    let (tx, rx): (Sender<TcpStream>, Receiver<TcpStream>) = std::sync::mpsc::channel();
    let rx = Arc::new(Mutex::new(rx));

    let mut worker_handles = Vec::with_capacity(workers);
    for _ in 0..workers.max(1) {
        let rx = Arc::clone(&rx);
        let root = root.clone();
        worker_handles.push(std::thread::spawn(move || loop {
            let stream = {
                let guard = match rx.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                guard.recv()
            };
            match stream {
                Ok(s) => handle_conn(s, &root),
                Err(_) => break,
            }
        }));
    }

    let accept = {
        let running = Arc::clone(&running);
        std::thread::spawn(move || {

            let tx = tx;
            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        if tx.send(stream).is_err() {
                            break;
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(40));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        })
    };

    logger.info(&format!("map server listening on http://{}/", addr));

    Ok(HttpServer {
        running,
        accept: Some(accept),
        workers: worker_handles,
        addr,
    })
}

fn handle_conn(mut stream: TcpStream, root: &Path) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));

    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let mut ok = false;
    while buf.len() < 16 * 1024 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if find_head_end(&buf).is_some() {
                    ok = true;
                    break;
                }
            }
            Err(_) => break,
        }
    }
    if !ok {
        return;
    }

    let head = match std::str::from_utf8(&buf) {
        Ok(s) => s,
        Err(_) => {
            let _ = send_simple(&mut stream, 400, "Bad Request");
            return;
        }
    };
    let request_line = head.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        let _ = send_simple(&mut stream, 405, "Method Not Allowed");
        return;
    }
    let head_only = method == "HEAD";

    let path_only = target.split(|c| c == '?' || c == '#').next().unwrap_or("/");

    if path_only == "/ws" {
        let upgrade = header_value(head, "upgrade")
            .map(|v| v.to_ascii_lowercase().contains("websocket"))
            .unwrap_or(false);
        if upgrade {
            if let Some(key) = header_value(head, "sec-websocket-key") {
                let key = key.trim().to_string();
                std::thread::spawn(move || crate::ws::serve(stream, key));
                return;
            }
        }
        let _ = send_simple(&mut stream, 400, "Bad Request");
        return;
    }

    let decoded = percent_decode(path_only);
    let rel = match safe_relative(&decoded) {
        Some(r) => r,
        None => {
            let _ = send_simple(&mut stream, 403, "Forbidden");
            return;
        }
    };

    let mut file_path = root.join(&rel);
    if rel.as_os_str().is_empty() {
        file_path = root.join("index.html");
    } else if file_path.is_dir() {
        file_path = file_path.join("index.html");
    }

    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let ctype = content_type(&file_path);
            let cache = cache_control(&file_path);
            let _ = send_bytes(&mut stream, 200, "OK", ctype, cache, &bytes, head_only);
        }
        Err(_) => {
            let _ = send_simple(&mut stream, 404, "Not Found");
        }
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    for line in head.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case(name) {
                return Some(v.trim());
            }
        }
    }
    None
}

fn safe_relative(path: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return None;
        }

        if seg.contains('\\') || seg.contains('\0') || seg.contains(':') {
            return None;
        }
        out.push(seg);
    }
    Some(out)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn cache_control(path: &Path) -> &'static str {

    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "public, max-age=31536000, immutable",
        _ => "no-store",
    }
}

fn send_bytes(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    ctype: &str,
    cache: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn send_simple(stream: &mut TcpStream, code: u16, reason: &str) -> std::io::Result<()> {
    let body = format!("{code} {reason}");
    send_bytes(
        stream,
        code,
        reason,
        "text/plain; charset=utf-8",
        "no-cache",
        body.as_bytes(),
        false,
    )
}
