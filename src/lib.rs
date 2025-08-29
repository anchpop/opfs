//! # OPFS - Origin Private File System
//!
//! A Rust implementation of the [Origin Private File System](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system) browser API.
//!
//! ## Usage
//!
//! The main entry point is the [`persistent`] module, which provides platform-agnostic
//! types that automatically resolve to the correct implementation:
//!
//! ```rust
//! use opfs::persistent::{DirectoryHandle, FileHandle, WritableFileStream, app_specific_dir};
//! use opfs::{GetFileHandleOptions, CreateWritableOptions};
//! use opfs::persistent;
//!
//! // you must import the traits to call methods on the types
//! use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};
//!
//! // This code works on both native and web platforms
//! async fn example(dir: DirectoryHandle) -> persistent::Result<()> {
//!     let options = GetFileHandleOptions { create: true };
//!     let mut file = dir.get_file_handle_with_options("example.txt", &options).await?;
//!     
//!     let write_options = CreateWritableOptions { keep_existing_data: false };
//!     let mut writer = file.create_writable_with_options(&write_options).await?;
//!     
//!     writer.write_at_cursor_pos(b"Hello, world!".to_vec()).await?;
//!     writer.close().await?;
//!     
//!     let data = file.read().await?;
//!     println!("File contents: {:?}", String::from_utf8(data));
//!     
//!     Ok(())
//! }
//!
//! async fn use_example() -> persistent::Result<()> {
//!     let directory: DirectoryHandle = app_specific_dir().await?;
//!     example(directory).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Platform-Specific Modules
//!
//! For advanced use cases, you can also access platform-specific implementations directly:
//!
//! - [`native`] - Native file system operations using `tokio::fs`
//! - [`web`] - Web platform operations using OPFS APIs
//! - [`memory`] - In-memory filesystem for use in tests (or when persistence isn't necessary)

pub mod memory;
pub mod persistent;

#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

use futures::Stream;
use std::fmt::Debug;
use std::ops::RangeBounds;

mod private {
    pub trait Sealed {}
}

pub struct GetFileHandleOptions {
    pub create: bool,
}

pub struct GetDirectoryHandleOptions {
    pub create: bool,
}

pub struct CreateWritableOptions {
    pub keep_existing_data: bool,
}

pub struct FileSystemRemoveOptions {
    pub recursive: bool,
}

#[derive(Debug, Clone)]
pub enum WriteCommandType {
    Write,
    Seek,
    Truncate,
}

#[derive(Debug, Clone)]
pub struct WriteParams {
    pub command_type: WriteCommandType,
    pub data: Option<Vec<u8>>,
    pub position: Option<usize>,
    pub size: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum DirectoryEntry<Directory, File> {
    File(File),
    Directory(Directory),
}

pub trait DirectoryHandle: Debug + Sized + private::Sealed {
    type Error: Debug;
    type FileHandleT: FileHandle<Error = Self::Error>;

    fn get_file_handle_with_options(
        &self,
        name: &str,
        options: &GetFileHandleOptions,
    ) -> impl std::future::Future<Output = Result<Self::FileHandleT, Self::Error>>;

    fn get_directory_handle_with_options(
        &self,
        name: &str,
        options: &GetDirectoryHandleOptions,
    ) -> impl std::future::Future<Output = Result<Self, Self::Error>>;

    fn remove_entry(
        &mut self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;

    fn remove_entry_with_options(
        &mut self,
        name: &str,
        options: &FileSystemRemoveOptions,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;

    #[allow(clippy::type_complexity)] // not sure how to improve this
    fn entries(
        &self,
    ) -> impl std::future::Future<
        Output = Result<
            impl Stream<Item = Result<(String, DirectoryEntry<Self, Self::FileHandleT>), Self::Error>>,
            Self::Error,
        >,
    >;
}

pub trait FileHandle: Debug + private::Sealed {
    type Error: Debug;
    type WritableFileStreamT: WritableFileStream<Error = Self::Error>;

    fn create_writable_with_options(
        &mut self,
        options: &CreateWritableOptions,
    ) -> impl std::future::Future<Output = Result<Self::WritableFileStreamT, Self::Error>>;

    fn read(&self) -> impl std::future::Future<Output = Result<Vec<u8>, Self::Error>>;

    fn read_range<R: RangeBounds<usize> + Send>(
        &self,
        range: R,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, Self::Error>>;

    fn size(&self) -> impl std::future::Future<Output = Result<usize, Self::Error>>;
}

pub trait WritableFileStream: Debug + private::Sealed {
    type Error: Debug;

    fn write_at_cursor_pos(
        &mut self,
        data: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;

    fn write_with_params(
        &mut self,
        params: &WriteParams,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>>;

    fn truncate(&mut self, size: usize) -> impl std::future::Future<Output = Result<(), Self::Error>>;

    fn close(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>>;

    fn seek(&mut self, offset: usize)
    -> impl std::future::Future<Output = Result<(), Self::Error>>;
}
