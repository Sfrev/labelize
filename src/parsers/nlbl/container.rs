//! The `.nlbl` container: a ZIP archive whose entries are AES-encrypted.
//!
//! labelize does not ship the archive password; callers supply it (the CLI
//! reads `LABELIZE_NLBL_PASSWORD`, optionally from a local `.env`).

use std::io::{Cursor, Read};

use crate::error::LabelizeError;

/// Upper bound on one decompressed entry. Real label documents are tens of
/// kilobytes; this only stops a hostile archive from exhausting memory.
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

/// One decrypted archive entry.
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
}

/// Decrypt and inflate every file entry of an `.nlbl` archive.
pub fn read_entries(bytes: &[u8], password: &[u8]) -> Result<Vec<Entry>, LabelizeError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| LabelizeError::Parse(format!("not an .nlbl (ZIP) file: {e}")))?;

    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let file = if password.is_empty() {
            archive.by_index(i)
        } else {
            archive.by_index_decrypt(i, password)
        };
        let file = file.map_err(|e| match e {
            zip::result::ZipError::InvalidPassword => {
                LabelizeError::Parse("wrong .nlbl password".to_string())
            }
            zip::result::ZipError::UnsupportedArchive(msg) if msg.contains("Password") => {
                LabelizeError::Parse(
                    ".nlbl entries are encrypted: a password is required".to_string(),
                )
            }
            e => LabelizeError::Parse(format!(".nlbl entry {i}: {e}")),
        })?;
        if file.is_dir() {
            continue;
        }
        let name = String::from_utf8_lossy(file.name_raw()).into_owned();
        if file.size() > MAX_ENTRY_BYTES {
            return Err(LabelizeError::Parse(format!(
                ".nlbl entry '{name}' is too large ({} bytes)",
                file.size()
            )));
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|e| LabelizeError::Parse(format!(".nlbl entry '{name}': {e}")))?;
        entries.push(Entry { name, data });
    }
    Ok(entries)
}
