use std::collections::BTreeMap;

use super::log;
use super::varint::Cursor;

const TAG_COMPARATOR: u32 = 1;
const TAG_LOG_NUMBER: u32 = 2;
const TAG_NEXT_FILE_NUMBER: u32 = 3;
const TAG_LAST_SEQUENCE: u32 = 4;
const TAG_COMPACT_POINTER: u32 = 5;
const TAG_DELETED_FILE: u32 = 6;
const TAG_NEW_FILE: u32 = 7;
const TAG_PREV_LOG_NUMBER: u32 = 9;

#[derive(Debug, Default)]
pub struct LiveVersion {

    pub files: BTreeMap<u32, Vec<u64>>,
    pub log_number: u64,
    pub prev_log_number: u64,
}

pub fn replay(manifest_bytes: &[u8]) -> LiveVersion {
    let mut live = LiveVersion::default();

    for record in log::read_records(manifest_bytes) {
        let mut cur = Cursor::new(&record);
        loop {
            let Some(tag) = cur.varint32() else { break };
            let ok = match tag {
                TAG_COMPARATOR => cur.length_prefixed().is_some(),
                TAG_LOG_NUMBER => {
                    if let Some(n) = cur.varint64() {
                        live.log_number = n;
                        true
                    } else {
                        false
                    }
                }
                TAG_PREV_LOG_NUMBER => {
                    if let Some(n) = cur.varint64() {
                        live.prev_log_number = n;
                        true
                    } else {
                        false
                    }
                }
                TAG_NEXT_FILE_NUMBER | TAG_LAST_SEQUENCE => cur.varint64().is_some(),
                TAG_COMPACT_POINTER => {
                    cur.varint32().is_some() && cur.length_prefixed().is_some()
                }
                TAG_DELETED_FILE => {
                    let level = cur.varint32();
                    let num = cur.varint64();
                    if let (Some(level), Some(num)) = (level, num) {
                        if let Some(v) = live.files.get_mut(&level) {
                            v.retain(|&f| f != num);
                        }
                        true
                    } else {
                        false
                    }
                }
                TAG_NEW_FILE => {
                    let level = cur.varint32();
                    let num = cur.varint64();
                    let size = cur.varint64();
                    let smallest = cur.length_prefixed();
                    let largest = cur.length_prefixed();
                    if let (Some(level), Some(num), Some(_), Some(_), Some(_)) =
                        (level, num, size, smallest, largest)
                    {
                        live.files.entry(level).or_default().push(num);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if !ok {

                break;
            }
        }
    }

    for v in live.files.values_mut() {
        v.sort_unstable();
        v.dedup();
    }
    live
}
