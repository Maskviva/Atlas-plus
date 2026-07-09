const BLOCK_SIZE: usize = 32768;
const HEADER_SIZE: usize = 7;

const T_FULL: u8 = 1;
const T_FIRST: u8 = 2;
const T_MIDDLE: u8 = 3;
const T_LAST: u8 = 4;

pub fn read_records(data: &[u8]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    let mut pending: Option<Vec<u8>> = None;

    let mut pos = 0usize;
    while pos + HEADER_SIZE <= data.len() {
        let block_off = pos % BLOCK_SIZE;
        let block_left = BLOCK_SIZE - block_off;
        if block_left < HEADER_SIZE {

            pos += block_left;
            continue;
        }

        let len = u16::from_le_bytes([data[pos + 4], data[pos + 5]]) as usize;
        let typ = data[pos + 6];
        let start = pos + HEADER_SIZE;
        let end = start + len;
        if end > data.len() || len > block_left - HEADER_SIZE {
            break;
        }
        let payload = &data[start..end];
        pos = end;

        match typ {
            T_FULL => {
                pending = None;
                records.push(payload.to_vec());
            }
            T_FIRST => {
                pending = Some(payload.to_vec());
            }
            T_MIDDLE => {
                if let Some(ref mut p) = pending {
                    p.extend_from_slice(payload);
                }
            }
            T_LAST => {
                if let Some(mut p) = pending.take() {
                    p.extend_from_slice(payload);
                    records.push(p);
                }
            }
            0 => {

                if len == 0 {

                    let block_off = pos % BLOCK_SIZE;
                    if block_off != 0 {
                        pos += BLOCK_SIZE - block_off;
                    }
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    records
}
