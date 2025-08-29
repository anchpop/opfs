//! Example demonstrating advanced file operations with the OPFS library

use opfs::persistent::{DirectoryHandle, app_specific_dir};
use opfs::{
    CreateWritableOptions, GetFileHandleOptions, 
    WriteParams, WriteCommandType,
    DirectoryHandle as _, FileHandle as _, WritableFileStream as _
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the app-specific directory
    let dir: DirectoryHandle = app_specific_dir().await?;
    
    // Create a file
    let options = GetFileHandleOptions { create: true };
    let mut file = dir.get_file_handle_with_options("example.txt", &options).await?;
    
    // Write some initial content
    let write_options = CreateWritableOptions { keep_existing_data: false };
    let mut writer = file.create_writable_with_options(&write_options).await?;
    
    writer.write_at_cursor_pos(b"Hello, World! This is a test file.".to_vec()).await?;
    writer.close().await?;
    
    // Demonstrate reading ranges
    println!("Full file content: {:?}", String::from_utf8(file.read().await?)?);
    
    // Read the first 5 bytes
    println!("First 5 bytes: {:?}", String::from_utf8(file.read_range(0..5).await?)?);
    
    // Read from byte 7 to the end
    println!("From byte 7 to end: {:?}", String::from_utf8(file.read_range(7..).await?)?);
    
    // Read bytes 7-12 (inclusive)
    println!("Bytes 7-12 inclusive: {:?}", String::from_utf8(file.read_range(7..=12).await?)?);
    
    // Read everything using RangeFull
    println!("Everything: {:?}", String::from_utf8(file.read_range(..).await?)?);
    
    // Demonstrate advanced write operations with WriteParams
    let mut writer = file.create_writable_with_options(&CreateWritableOptions { 
        keep_existing_data: true 
    }).await?;
    
    // Write at a specific position
    let params = WriteParams {
        command_type: WriteCommandType::Write,
        data: Some(b"RUST".to_vec()),
        position: Some(7),
        size: None,
    };
    writer.write_with_params(&params).await?;
    
    // Truncate the file
    let params = WriteParams {
        command_type: WriteCommandType::Truncate,
        data: None,
        position: None,
        size: Some(20),
    };
    writer.write_with_params(&params).await?;
    
    // Seek and write
    let params = WriteParams {
        command_type: WriteCommandType::Seek,
        data: None,
        position: Some(15),
        size: None,
    };
    writer.write_with_params(&params).await?;
    writer.write_at_cursor_pos(b"!!!".to_vec()).await?;
    
    writer.close().await?;
    
    println!("Final file content: {:?}", String::from_utf8(file.read().await?)?);
    println!("File size: {} bytes", file.size().await?);
    
    Ok(())
}