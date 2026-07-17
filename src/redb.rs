//! A redb storage backend backed by an OPFS synchronous access file.

use std::io;

use wasm_bindgen::JsValue;

use crate::{sync_access::SyncFile, web::FileHandle};

/// A redb storage backend backed by a synchronous OPFS file.
#[derive(Debug)]
pub struct OpfsBackend {
    file: SyncFile,
}

impl OpfsBackend {
    /// Opens an exclusive synchronous OPFS access handle for `file`.
    ///
    /// The caller must be running in a dedicated worker.
    pub async fn open(file: &FileHandle) -> Result<Self, JsValue> {
        Ok(Self {
            file: SyncFile::open(file).await?,
        })
    }

    /// Wraps an already-open synchronous OPFS file.
    pub fn new(file: SyncFile) -> Self {
        Self { file }
    }
}

impl redb::StorageBackend for OpfsBackend {
    fn len(&self) -> io::Result<u64> {
        self.file.len()
    }

    fn read(&self, offset: u64, output: &mut [u8]) -> io::Result<()> {
        let read = self.file.read_at(offset, output)?;
        if read == output.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("read {read} bytes, expected {}", output.len()),
            ))
        }
    }

    fn set_len(&self, len: u64) -> io::Result<()> {
        self.file.set_len(len)
    }

    fn sync_data(&self) -> io::Result<()> {
        self.file.flush()
    }

    fn write(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        let written = self.file.write_at(offset, data)?;
        if written == data.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!("wrote {written} bytes, expected {}", data.len()),
            ))
        }
    }

    fn close(&self) -> io::Result<()> {
        self.file.close()
    }
}
