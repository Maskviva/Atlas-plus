use std::io::Read;

use super::varint::Cursor;

const FOOTER_SIZE: usize = 48;
const MAGIC: u64 = 0xdb4775248b80fb57;

pub const TYPE_DELETION: u8 = 0;
pub const TYPE_VALUE: u8 = 1;

#[derive(Debug, Clone, Copy)]
struct BlockHandle {
    offset: u64,
    size: u64,
}

fn read_handle(cur: &mut Cursor) -> Option<BlockHandle> {
    Some(BlockHandle {
        offset: cur.varint64()?,
        size: cur.varint64()?,
    })
}

fn decompress(kind: u8, data: &[u8]) -> Option<Vec<u8>> {
    match kind {
        0 => Some(data.to_vec()),
        2 => {

            let mut out = Vec::with_capacity(data.len() * 3);
            flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).ok()?;
            Some(out)
        }
        4 => {

            let mut out = Vec::with_capacity(data.len() * 3);
            flate2::read::DeflateDecoder::new(data).read_to_end(&mut out).ok()?;
            Some(out)
        }
        1 => None,
        _ => None,
    }
}

fn read_block(file: &[u8], handle: BlockHandle) -> Option<Vec<u8>> {
    let off = handle.offset as usize;
    let size = handle.size as usize;
    if off + size + 5 > file.len() {

        if off + size > file.len() {
            return None;
        }

        return decompress(0, &file[off..off + size]);
    }
    let kind = file[off + size];
    decompress(kind, &file[off..off + size])
}

fn iter_block_entries(block: &[u8], mut f: impl FnMut(&[u8], &[u8])) {
    if block.len() < 4 {
        return;
    }
    let n_restarts =
        u32::from_le_bytes(block[block.len() - 4..].try_into().unwrap()) as usize;
    let restarts_size = 4 + n_restarts * 4;
    if restarts_size > block.len() {
        return;
    }
    let data_end = block.len() - restarts_size;

    let mut cur = Cursor::new(&block[..data_end]);
    let mut key: Vec<u8> = Vec::new();
    while cur.remaining() > 0 {
        let Some(shared) = cur.varint32() else { break };
        let Some(non_shared) = cur.varint32() else { break };
        let Some(value_len) = cur.varint32() else { break };
        let Some(delta) = cur.take(non_shared as usize) else { break };
        let Some(value) = cur.take(value_len as usize) else { break };
        let shared = shared as usize;
        if shared > key.len() {
            break;
        }
        key.truncate(shared);
        key.extend_from_slice(delta);
        f(&key, value);
    }
}

pub fn iter_table(file: &[u8], mut f: impl FnMut(&[u8], u64, u8, &[u8])) -> Result<(), String> {
    if file.len() < FOOTER_SIZE {
        return Err("file too small".into());
    }
    let footer = &file[file.len() - FOOTER_SIZE..];
    let magic = u64::from_le_bytes(footer[FOOTER_SIZE - 8..].try_into().unwrap());
    if magic != MAGIC {
        return Err("bad table magic".into());
    }
    let mut cur = Cursor::new(&footer[..FOOTER_SIZE - 8]);
    let _metaindex = read_handle(&mut cur).ok_or("bad metaindex handle")?;
    let index_handle = read_handle(&mut cur).ok_or("bad index handle")?;

    let index_block =
        read_block(file, index_handle).ok_or("failed to read index block")?;

    let mut handles: Vec<BlockHandle> = Vec::new();
    iter_block_entries(&index_block, |_key, value| {
        let mut c = Cursor::new(value);
        if let Some(h) = read_handle(&mut c) {
            handles.push(h);
        }
    });

    for handle in handles {
        let Some(block) = read_block(file, handle) else { continue };
        iter_block_entries(&block, |ikey, value| {

            if ikey.len() < 8 {
                return;
            }
            let (user_key, trailer) = ikey.split_at(ikey.len() - 8);
            let packed = u64::from_le_bytes(trailer.try_into().unwrap());
            let seq = packed >> 8;
            let vtype = (packed & 0xff) as u8;
            f(user_key, seq, vtype, value);
        });
    }
    Ok(())
}
