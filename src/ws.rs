use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use image::{ImageEncoder, RgbaImage};
use serde_json::{json, Value};

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let ml = (msg.len() as u64) * 8;
    let mut data = msg.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&ml.to_be_bytes());
    let mut w = [0u32; 80];
    for block in data.chunks_exact(64) {
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for t in 16..80 {
            w[t] = (w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (t, &wt) in w.iter().enumerate() {
            let (f, k) = if t < 20 {
                ((b & c) | ((!b) & d), 0x5A827999u32)
            } else if t < 40 {
                (b ^ c ^ d, 0x6ED9EBA1)
            } else if t < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1BBCDC)
            } else {
                (b ^ c ^ d, 0xCA62C1D6)
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wt);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for i in 0..5 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() { data[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as usize } else { 0 };
        out.push(A[b0 >> 2] as char);
        out.push(A[((b0 & 3) << 4) | (b1 >> 4)] as char);
        out.push(if i + 1 < data.len() { A[((b1 & 15) << 2) | (b2 >> 6)] as char } else { '=' });
        out.push(if i + 2 < data.len() { A[b2 & 63] as char } else { '=' });
        i += 3;
    }
    out
}

pub fn accept_key(sec_key: &str) -> String {
    let mut v = sec_key.as_bytes().to_vec();
    v.extend_from_slice(GUID.as_bytes());
    b64(&sha1(&v))
}

#[derive(Clone)]
enum Out {
    Text(Arc<String>),
    Binary(Arc<Vec<u8>>),
}

struct Client {
    id: u64,
    tx: Sender<Out>,
}

struct Hub {
    clients: Vec<Client>,
    next_id: u64,
    last_map: Option<Arc<String>>,
    last_players: Option<Arc<String>>,
    tiles: HashMap<(i32, i32, i32, i32), Arc<Vec<u8>>>,
}

fn hub() -> &'static Mutex<Hub> {
    static S: OnceLock<Mutex<Hub>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(Hub {
            clients: Vec::new(),
            next_id: 0,
            last_map: None,
            last_players: None,
            tiles: HashMap::new(),
        })
    })
}

fn register() -> (u64, Receiver<Out>) {
    let (tx, rx) = channel::<Out>();
    let mut h = lk(hub());
    let id = h.next_id;
    h.next_id += 1;
    if let Some(m) = &h.last_map {
        let _ = tx.send(Out::Text(m.clone()));
    }
    for t in h.tiles.values() {
        let _ = tx.send(Out::Binary(t.clone()));
    }
    if let Some(p) = &h.last_players {
        let _ = tx.send(Out::Text(p.clone()));
    }
    h.clients.push(Client { id, tx });
    (id, rx)
}

fn unregister(id: u64) {
    lk(hub()).clients.retain(|c| c.id != id);
}

fn fanout(h: &mut Hub, out: Out) {
    h.clients.retain(|c| c.tx.send(out.clone()).is_ok());
}

pub fn publish_map(manifest: &Value) {
    let msg = Arc::new(json!({ "type": "map", "data": manifest }).to_string());
    let mut h = lk(hub());
    h.last_map = Some(msg.clone());
    fanout(&mut h, Out::Text(msg));
}

pub fn publish_players(players: &Value, version: u64) {
    let msg = Arc::new(json!({ "type": "players", "players": players, "version": version }).to_string());
    let mut h = lk(hub());
    h.last_players = Some(msg.clone());
    fanout(&mut h, Out::Text(msg));
}

fn encode_png(img: &RgbaImage) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(img.as_raw(), img.width(), img.height(), image::ColorType::Rgba8)
        .ok()?;
    Some(buf)
}

pub fn publish_tile(dim: i32, z: u32, tx: i32, tz: i32, img: &RgbaImage) {
    let Some(png) = encode_png(img) else { return };
    let zi = z as i32;
    let mut frame = Vec::with_capacity(16 + png.len());
    frame.extend_from_slice(&dim.to_be_bytes());
    frame.extend_from_slice(&zi.to_be_bytes());
    frame.extend_from_slice(&tx.to_be_bytes());
    frame.extend_from_slice(&tz.to_be_bytes());
    frame.extend_from_slice(&png);
    let msg = Arc::new(frame);
    let mut h = lk(hub());
    h.tiles.insert((dim, zi, tx, tz), msg.clone());
    fanout(&mut h, Out::Binary(msg));
}

fn write_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut hdr = Vec::with_capacity(10);
    hdr.push(0x80 | opcode);
    let len = payload.len();
    if len < 126 {
        hdr.push(len as u8);
    } else if len <= 0xFFFF {
        hdr.push(126);
        hdr.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        hdr.push(127);
        hdr.extend_from_slice(&(len as u64).to_be_bytes());
    }
    stream.write_all(&hdr)?;
    stream.write_all(payload)?;
    stream.flush()
}

fn read_exact_eof(stream: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut read = 0;
    while read < buf.len() {
        match stream.read(&mut buf[read..]) {
            Ok(0) => return Ok(false),
            Ok(n) => read += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

enum Frame {
    Ping(Vec<u8>),
    Close,
    Other,
}

fn read_frame(stream: &mut TcpStream) -> Option<Frame> {
    let mut h = [0u8; 2];
    if !read_exact_eof(stream, &mut h).ok()? {
        return None;
    }
    let opcode = h[0] & 0x0F;
    let masked = h[1] & 0x80 != 0;
    let mut len = (h[1] & 0x7F) as usize;
    if len == 126 {
        let mut e = [0u8; 2];
        if !read_exact_eof(stream, &mut e).ok()? {
            return None;
        }
        len = u16::from_be_bytes(e) as usize;
    } else if len == 127 {
        let mut e = [0u8; 8];
        if !read_exact_eof(stream, &mut e).ok()? {
            return None;
        }
        len = u64::from_be_bytes(e) as usize;
    }
    if len > 1 << 20 {
        return Some(Frame::Close);
    }
    let mut mask = [0u8; 4];
    if masked && !read_exact_eof(stream, &mut mask).ok()? {
        return None;
    }
    let mut payload = vec![0u8; len];
    if len > 0 && !read_exact_eof(stream, &mut payload).ok()? {
        return None;
    }
    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i & 3];
        }
    }
    match opcode {
        0x8 => Some(Frame::Close),
        0x9 => Some(Frame::Ping(payload)),
        _ => Some(Frame::Other),
    }
}

pub fn serve(mut stream: TcpStream, sec_key: String) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(None);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(20)));
    let resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_key(&sec_key)
    );
    if stream.write_all(resp.as_bytes()).is_err() {
        return;
    }

    let write_half = match stream.try_clone() {
        Ok(s) => Arc::new(Mutex::new(s)),
        Err(_) => return,
    };
    let (id, rx) = register();

    let writer = {
        let write_half = write_half.clone();
        let peer = stream.try_clone().ok();
        std::thread::spawn(move || {
            for msg in rx {
                let ok = {
                    let mut s = lk(&write_half);
                    match msg {
                        Out::Text(t) => write_frame(&mut s, 0x1, t.as_bytes()).is_ok(),
                        Out::Binary(b) => write_frame(&mut s, 0x2, &b).is_ok(),
                    }
                };
                if !ok {
                    if let Some(p) = &peer {
                        let _ = p.shutdown(Shutdown::Both);
                    }
                    break;
                }
            }
        })
    };

    loop {
        match read_frame(&mut stream) {
            Some(Frame::Ping(p)) => {
                let ok = {
                    let mut s = lk(&write_half);
                    write_frame(&mut s, 0xA, &p).is_ok()
                };
                if !ok {
                    break;
                }
            }
            Some(Frame::Other) => {}
            Some(Frame::Close) | None => break,
        }
    }

    unregister(id);
    let _ = stream.shutdown(Shutdown::Both);
    let _ = writer.join();
}
