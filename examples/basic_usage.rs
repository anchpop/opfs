//! Basic usage example for the OPFS crate
//!
//! This example demonstrates the fundamental operations:
//! - Creating/getting a file handle
//! - Writing data to a file
//! - Reading data from a file
//! - Directory operations

use opfs::persistent::{DirectoryHandle, FileHandle, WritableFileStream, app_specific_dir};
use opfs::{CreateWritableOptions, GetFileHandleOptions};

// Import the traits to call methods on the types
use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the app-specific directory (works on both native and web)
    let dir: DirectoryHandle = app_specific_dir().await?;

    // Create or get a file handle
    let options = GetFileHandleOptions { create: true };
    let mut file: FileHandle = dir
        .get_file_handle_with_options("hello.txt", &options)
        .await?;

    // Write some data to the file
    let write_options = CreateWritableOptions {
        keep_existing_data: false,
    };
    let mut writer: WritableFileStream = file.create_writable_with_options(&write_options).await?;

    let message = b"Hello from OPFS! This works on both native and web platforms.";
    writer.write_at_cursor_pos(message).await?;
    writer.close().await?;

    // Read the data back
    let data = file.read().await?;
    let content = String::from_utf8(data)?;

    println!("File contents: {}", content);
    println!("File size: {} bytes", file.size().await?);

    Ok(())
}
