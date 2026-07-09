use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::log;
use super::manifest;
use super::table;
use super::varint::Cursor;

pub type Snapshot = HashMap<Vec<u8>, Vec<u8>>;

pub fn load(db_dir: &Path, filter: impl Fn(&[u8]) -> bool) -> Result<Snapshot, String> {
    if !db_dir.is_dir() {
        return Err(format!("{} is not a directory", db_dir.display()));
    }
    let current = fs::read_to_string(db_dir.join("CURRENT"))
        .map_err(|e| format!("failed to read CURRENT: {e}"))?;
    let manifest_name = current.trim();
    let manifest_bytes = fs::read(db_dir.join(manifest_name))
        .map_err(|e| format!("failed to read {manifest_name}: {e}"))?;
    let live = manifest::replay(&manifest_bytes);

    let mut snapshot: Snapshot = HashMap::new();
    let mut apply = |key: &[u8], vtype: u8, value: &[u8]| {
        if vtype == table::TYPE_DELETION {
            snapshot.remove(key);
        } else if filter(key) {
            snapshot.insert(key.to_vec(), value.to_vec());
        }
    };
    let mut levels: Vec<u32> = live.files.keys().copied().collect();
    levels.sort_unstable_by(|a, b| b.cmp(a));

    let mut table_count = 0usize;
    for level in levels {
        let files = &live.files[&level];

        for &num in files {
            let Some(bytes) = read_table_file(db_dir, num) else {
                eprintln!("warn: missing table file {:06}.ldb", num);
                continue;
            };
            table_count += 1;

            let mut last_key: Option<Vec<u8>> = None;
            let res = table::iter_table(&bytes, |ukey, _seq, vtype, value| {
                if last_key.as_deref() == Some(ukey) {
                    return;
                }
                last_key = Some(ukey.to_vec());
                apply(ukey, vtype, value);
            });
            if let Err(e) = res {
                eprintln!("warn: table {:06}: {e}", num);
            }
        }
    }
    let min_log = if live.prev_log_number > 0 {
        live.prev_log_number
    } else {
        live.log_number
    };
    let mut logs: Vec<u64> = Vec::new();
    if let Ok(entries) = fs::read_dir(db_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".log") {
                if let Ok(num) = stem.parse::<u64>() {
                    if min_log == 0 || num >= min_log {
                        logs.push(num);
                    }
                }
            }
        }
    }
    logs.sort_unstable();

    for num in logs {
        let path = db_dir.join(format!("{:06}.log", num));
        let Ok(bytes) = fs::read(&path) else { continue };
        for record in log::read_records(&bytes) {
            apply_write_batch(&record, &mut apply);
        }
    }

    eprintln!(
        "leveldb: merged {} table files → {} live keys kept",
        table_count,
        snapshot.len()
    );

    Ok(snapshot)
}

fn read_table_file(dir: &Path, num: u64) -> Option<Vec<u8>> {
    for ext in ["ldb", "sst"] {
        let path = dir.join(format!("{:06}.{ext}", num));
        if let Ok(bytes) = fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

fn apply_write_batch(record: &[u8], apply: &mut impl FnMut(&[u8], u8, &[u8])) {
    if record.len() < 12 {
        return;
    }
    let count = u32::from_le_bytes(record[8..12].try_into().unwrap());
    let mut cur = Cursor::new(&record[12..]);
    for _ in 0..count {
        let Some(op) = cur.u8() else { return };
        match op {
            1 => {
                let Some(key) = cur.length_prefixed() else { return };
                let Some(value) = cur.length_prefixed() else { return };
                apply(key, table::TYPE_VALUE, value);
            }
            0 => {
                let Some(key) = cur.length_prefixed() else { return };
                apply(key, table::TYPE_DELETION, &[]);
            }
            _ => return,
        }
    }
}
