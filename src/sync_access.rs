//! Synchronous random-access OPFS files.
//!
//! This module is only available on wasm and must be used from a dedicated
//! worker, because browsers do not expose synchronous OPFS access handles on
//! the window or shared-worker globals.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt, io,
};

use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{FileSystemReadWriteOptions, FileSystemSyncAccessHandle};

use crate::web::FileHandle;

thread_local! {
    static NEXT_HANDLE: Cell<u32> = const { Cell::new(1) };
    static HANDLES: RefCell<HashMap<u32, FileSystemSyncAccessHandle>> = RefCell::new(HashMap::new());
}

/// A synchronous random-access file whose JavaScript handle remains local to
/// the worker that opened it.
///
/// This value contains only a numeric registry key, so it is `Send + Sync`
/// without claiming that its JavaScript handle can cross threads. Operations
/// from anywhere other than the creating worker return an I/O error.
pub struct SyncFile {
    key: u32,
}

impl SyncFile {
    /// Opens an exclusive synchronous access handle for `file`.
    ///
    /// The caller must be running in a dedicated worker. Opening is async, but
    /// all file operations after this constructor returns are synchronous.
    pub async fn open(file: &FileHandle) -> Result<Self, JsValue> {
        let handle = FileSystemSyncAccessHandle::from(
            JsFuture::from(file.inner().create_sync_access_handle()).await?,
        );
        let key = NEXT_HANDLE.with(|next| {
            let key = next.get();
            next.set(key.checked_add(1).expect("OPFS handle key overflow"));
            key
        });
        HANDLES.with(|handles| handles.borrow_mut().insert(key, handle));
        Ok(Self { key })
    }

    /// Returns the current file length.
    pub fn len(&self) -> io::Result<u64> {
        self.with_handle(|handle| handle.get_size())
            .and_then(f64_to_u64)
    }

    /// Returns whether the file is empty.
    pub fn is_empty(&self) -> io::Result<bool> {
        self.len().map(|len| len == 0)
    }

    /// Reads bytes at `offset`, returning the number of bytes read.
    pub fn read_at(&self, offset: u64, output: &mut [u8]) -> io::Result<usize> {
        let options = options_at(offset)?;
        self.with_handle(|handle| handle.read_with_u8_array_and_options(output, &options))
            .and_then(f64_to_usize)
    }

    /// Writes bytes at `offset`, returning the number of bytes written.
    pub fn write_at(&self, offset: u64, data: &[u8]) -> io::Result<usize> {
        let options = options_at(offset)?;
        self.with_handle(|handle| handle.write_with_u8_array_and_options(data, &options))
            .and_then(f64_to_usize)
    }

    /// Changes the file length to `len`.
    pub fn set_len(&self, len: u64) -> io::Result<()> {
        let len = u64_to_f64(len)?;
        self.with_handle(|handle| handle.truncate_with_f64(len))
    }

    /// Flushes buffered file data to OPFS.
    pub fn flush(&self) -> io::Result<()> {
        self.with_handle(FileSystemSyncAccessHandle::flush)
    }

    /// Flushes and closes the synchronous access handle.
    pub fn close(&self) -> io::Result<()> {
        HANDLES.with(|handles| {
            let handle = handles
                .borrow_mut()
                .remove(&self.key)
                .ok_or_else(closed_error)?;
            handle.flush().map_err(js_error)?;
            handle.close();
            Ok(())
        })
    }

    fn with_handle<T>(
        &self,
        operation: impl FnOnce(&FileSystemSyncAccessHandle) -> Result<T, JsValue>,
    ) -> io::Result<T> {
        HANDLES.with(|handles| {
            let handles = handles.borrow();
            let handle = handles.get(&self.key).ok_or_else(closed_error)?;
            operation(handle).map_err(js_error)
        })
    }
}

impl fmt::Debug for SyncFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SyncFile")
            .field("key", &self.key)
            .finish()
    }
}

impl Drop for SyncFile {
    fn drop(&mut self) {
        HANDLES.with(|handles| {
            if let Some(handle) = handles.borrow_mut().remove(&self.key) {
                let _ = handle.flush();
                handle.close();
            }
        });
    }
}

fn options_at(offset: u64) -> io::Result<FileSystemReadWriteOptions> {
    let options = FileSystemReadWriteOptions::new();
    options.set_at(u64_to_f64(offset)?);
    Ok(options)
}

fn closed_error() -> io::Error {
    io::Error::other("OPFS handle is closed or belongs to another worker")
}

fn js_error(error: JsValue) -> io::Error {
    io::Error::other(error.as_string().unwrap_or_else(|| format!("{error:?}")))
}

fn f64_to_u64(value: f64) -> io::Result<u64> {
    if value.is_finite() && value >= 0.0 && value.fract() == 0.0 && value <= u64::MAX as f64 {
        Ok(value as u64)
    } else {
        Err(io::Error::other(format!("invalid OPFS size: {value}")))
    }
}

fn f64_to_usize(value: f64) -> io::Result<usize> {
    let value = f64_to_u64(value)?;
    usize::try_from(value).map_err(io::Error::other)
}

fn u64_to_f64(value: u64) -> io::Result<f64> {
    const MAX_SAFE_INTEGER: u64 = (1_u64 << 53) - 1;
    if value <= MAX_SAFE_INTEGER {
        Ok(value as f64)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("OPFS offset or length exceeds JavaScript's safe integer range: {value}"),
        ))
    }
}
