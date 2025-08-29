use futures::Stream;
use std::sync::Arc;
use std::{io::SeekFrom, path::PathBuf};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::RwLock;

type DirectoryEntry = crate::DirectoryEntry<DirectoryHandle, FileHandle>;

#[derive(Clone, Debug)]
pub struct DirectoryHandle(PathBuf);

#[derive(Clone, Debug)]
pub struct FileHandle(PathBuf);

#[derive(Clone, Debug)]
pub struct WritableFileStream(Arc<RwLock<tokio::fs::File>>);

impl From<PathBuf> for DirectoryHandle {
    fn from(handle: PathBuf) -> Self {
        Self(handle)
    }
}

impl From<PathBuf> for FileHandle {
    fn from(handle: PathBuf) -> Self {
        Self(handle)
    }
}

impl From<tokio::fs::File> for WritableFileStream {
    fn from(handle: tokio::fs::File) -> Self {
        Self(Arc::new(RwLock::new(handle)))
    }
}

impl crate::private::Sealed for DirectoryHandle {}
impl crate::private::Sealed for FileHandle {}
impl crate::private::Sealed for WritableFileStream {}

impl crate::DirectoryHandle for DirectoryHandle {
    type Error = std::io::Error;
    type FileHandleT = FileHandle;

    async fn get_file_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetFileHandleOptions,
    ) -> Result<Self::FileHandleT, Self::Error> {
        let mut path = self.0.clone();
        path.push(name);

        // Make sure the file exists
        let _ = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(options.create)
            .open(&path)
            .await?;

        Ok(FileHandle(path))
    }

    async fn get_directory_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetDirectoryHandleOptions,
    ) -> Result<Self, Self::Error> {
        let mut path = self.0.clone();
        path.push(name);

        if options.create {
            tokio::fs::create_dir_all(&path).await?;
        } else if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory '{}' not found", name),
            ));
        }

        Ok(DirectoryHandle(path))
    }

    async fn remove_entry(&mut self, name: &str) -> Result<(), Self::Error> {
        let mut path = self.0.clone();
        path.push(name);

        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.is_file() {
            tokio::fs::remove_file(&path).await?;
        } else if metadata.is_dir() {
            tokio::fs::remove_dir(&path).await?;
        }

        Ok(())
    }

    async fn remove_entry_with_options(
        &mut self,
        name: &str,
        options: &crate::FileSystemRemoveOptions,
    ) -> Result<(), Self::Error> {
        let mut path = self.0.clone();
        path.push(name);

        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.is_file() {
            tokio::fs::remove_file(&path).await?;
        } else if metadata.is_dir() {
            if options.recursive {
                tokio::fs::remove_dir_all(&path).await?;
            } else {
                tokio::fs::remove_dir(&path).await?;
            }
        }

        Ok(())
    }

    async fn entries(
        &self,
    ) -> Result<impl Stream<Item = Result<(String, DirectoryEntry), Self::Error>>, Self::Error>
    {
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&self.0).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata().await?;

            let dir_entry = if metadata.is_file() {
                DirectoryEntry::File(FileHandle(entry.path()))
            } else if metadata.is_dir() {
                DirectoryEntry::Directory(DirectoryHandle(entry.path()))
            } else {
                continue; // Skip other types like symlinks
            };

            entries.push(Ok((name, dir_entry)));
        }

        Ok(futures::stream::iter(entries))
    }
}

impl crate::FileHandle for FileHandle {
    type Error = std::io::Error;
    type WritableFileStreamT = WritableFileStream;

    async fn create_writable_with_options(
        &mut self,
        options: &crate::CreateWritableOptions,
    ) -> Result<Self::WritableFileStreamT, Self::Error> {
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(!options.keep_existing_data)
            .open(&self.0)
            .await?;

        Ok(WritableFileStream(Arc::new(RwLock::new(file))))
    }

    async fn read(&self) -> Result<Vec<u8>, Self::Error> {
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(&self.0).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;
        Ok(buffer)
    }

    async fn read_range<R: std::ops::RangeBounds<usize> + Send>(
        &self,
        range: R,
    ) -> Result<Vec<u8>, Self::Error> {
        use std::ops::Bound;
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let mut file = tokio::fs::File::open(&self.0).await?;
        let file_size = file.metadata().await?.len() as usize;
        
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => file_size,
        };
        
        if start >= file_size {
            return Ok(Vec::new());
        }
        
        let actual_end = end.min(file_size);
        let bytes_to_read = actual_end.saturating_sub(start);
        
        if bytes_to_read == 0 {
            return Ok(Vec::new());
        }
        
        file.seek(SeekFrom::Start(start as u64)).await?;
        let mut buffer = vec![0; bytes_to_read];
        file.read_exact(&mut buffer).await?;
        Ok(buffer)
    }

    async fn size(&self) -> Result<usize, Self::Error> {
        let metadata = tokio::fs::metadata(&self.0).await?;
        Ok(metadata.len() as usize)
    }
}

impl crate::WritableFileStream for WritableFileStream {
    type Error = std::io::Error;

    async fn write_at_cursor_pos(&mut self, data: Vec<u8>) -> Result<(), Self::Error> {
        let mut file = self.0.write().await;
        file.write_all(&data).await?;
        Ok(())
    }

    async fn write_with_params(&mut self, params: &crate::WriteParams) -> Result<(), Self::Error> {
        use crate::WriteCommandType;
        
        let mut file = self.0.write().await;
        
        match params.command_type {
            WriteCommandType::Write => {
                if let Some(data) = &params.data {
                    if let Some(position) = params.position {
                        file.seek(SeekFrom::Start(position as u64)).await?;
                    }
                    file.write_all(data).await?;
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Write command requires data",
                    ));
                }
            }
            WriteCommandType::Seek => {
                if let Some(position) = params.position {
                    file.seek(SeekFrom::Start(position as u64)).await?;
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Seek command requires position",
                    ));
                }
            }
            WriteCommandType::Truncate => {
                if let Some(size) = params.size {
                    file.set_len(size as u64).await?;
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Truncate command requires size",
                    ));
                }
            }
        }
        Ok(())
    }

    async fn truncate(&mut self, size: usize) -> Result<(), Self::Error> {
        let file = self.0.write().await;
        file.set_len(size as u64).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        let mut file = self.0.write().await;
        file.shutdown().await?;
        Ok(())
    }

    async fn seek(&mut self, offset: usize) -> Result<(), Self::Error> {
        let mut file = self.0.write().await;
        file.seek(SeekFrom::Start(offset as u64)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CreateWritableOptions, DirectoryHandle as _, FileHandle as _, GetFileHandleOptions,
        WritableFileStream as _,
    };
    use futures::StreamExt;
    use tempfile::TempDir;

    async fn setup_temp_dir() -> (TempDir, DirectoryHandle) {
        let temp_dir = TempDir::new().unwrap();
        let dir_handle = DirectoryHandle(temp_dir.path().to_path_buf());
        (temp_dir, dir_handle)
    }

    #[tokio::test]
    async fn test_create_and_read_file() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let mut file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .unwrap();

        let data = b"Hello, world!".to_vec();
        writer.write_at_cursor_pos(data.clone()).await.unwrap();
        writer.close().await.unwrap();

        let read_data = file.read().await.unwrap();
        assert_eq!(read_data, data);
        assert_eq!(file.size().await.unwrap(), data.len());
    }

    #[tokio::test]
    async fn test_file_not_found() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: false };

        let result = dir
            .get_file_handle_with_options("nonexistent.txt", &options)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_entry() {
        let (_temp_dir, mut dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let _file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        dir.remove_entry("test.txt").await.unwrap();

        let result = dir
            .get_file_handle_with_options("test.txt", &GetFileHandleOptions { create: false })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_entries_empty() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let entries_stream = dir.entries().await.unwrap();
        let entries: Vec<_> = entries_stream.collect().await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_entries_with_files() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let _file1 = dir
            .get_file_handle_with_options("file1.txt", &options)
            .await
            .unwrap();
        let _file2 = dir
            .get_file_handle_with_options("file2.txt", &options)
            .await
            .unwrap();

        let entries_stream = dir.entries().await.unwrap();
        let entries: Vec<_> = entries_stream.collect().await;

        assert_eq!(entries.len(), 2);

        let mut names: Vec<_> = entries.into_iter().map(|r| r.unwrap().0).collect();
        names.sort();
        assert_eq!(names, vec!["file1.txt", "file2.txt"]);
    }

    #[tokio::test]
    async fn test_entries_with_subdirectory() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        // Create a file
        let _file = dir
            .get_file_handle_with_options("file.txt", &options)
            .await
            .unwrap();

        // Create a subdirectory
        let mut subdir_path = dir.0.clone();
        subdir_path.push("subdir");
        tokio::fs::create_dir(&subdir_path).await.unwrap();

        let entries_stream = dir.entries().await.unwrap();
        let entries: Vec<_> = entries_stream.collect().await;

        assert_eq!(entries.len(), 2);

        let mut items: Vec<_> = entries
            .into_iter()
            .map(|r| {
                let (name, entry) = r.unwrap();
                let is_dir = matches!(entry, DirectoryEntry::Directory(_));
                (name, is_dir)
            })
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(items[0].0, "file.txt");
        assert!(!items[0].1); // is file
        assert_eq!(items[1].0, "subdir");
        assert!(items[1].1); // is directory
    }

    #[tokio::test]
    async fn test_seek_and_write() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let mut file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .unwrap();

        writer.write_at_cursor_pos(b"Hello".to_vec()).await.unwrap();
        writer.seek(0).await.unwrap();
        writer.write_at_cursor_pos(b"Hi".to_vec()).await.unwrap();
        writer.close().await.unwrap();

        let data = file.read().await.unwrap();
        assert_eq!(data, b"Hillo"); // "Hi" overwrites first 2 chars
    }

    #[tokio::test]
    async fn test_keep_existing_data() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let mut file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        // Write initial data
        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .unwrap();
        writer.write_at_cursor_pos(b"Hello".to_vec()).await.unwrap();
        writer.close().await.unwrap();

        // Write more data keeping existing
        let keep_options = CreateWritableOptions {
            keep_existing_data: true,
        };
        let mut writer2 = file
            .create_writable_with_options(&keep_options)
            .await
            .unwrap();
        writer2
            .write_at_cursor_pos(b" World".to_vec())
            .await
            .unwrap();
        writer2.close().await.unwrap();

        let data = file.read().await.unwrap();
        assert_eq!(data, b" World"); // Overwrites from beginning when keeping data
    }

    #[tokio::test]
    async fn test_truncate_existing_data() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let mut file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        // Write initial data
        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .unwrap();
        writer
            .write_at_cursor_pos(b"Hello World".to_vec())
            .await
            .unwrap();
        writer.close().await.unwrap();

        // Truncate and write new data
        let truncate_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer2 = file
            .create_writable_with_options(&truncate_options)
            .await
            .unwrap();
        writer2.write_at_cursor_pos(b"Hi".to_vec()).await.unwrap();
        writer2.close().await.unwrap();

        let data = file.read().await.unwrap();
        assert_eq!(data, b"Hi");
    }

    #[tokio::test]
    async fn test_read_range() {
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let mut file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .unwrap();
        writer.write_at_cursor_pos(b"Hello, World!".to_vec()).await.unwrap();
        writer.close().await.unwrap();

        // Test various range types
        assert_eq!(file.read_range(0..5).await.unwrap(), b"Hello");
        assert_eq!(file.read_range(7..).await.unwrap(), b"World!");
        assert_eq!(file.read_range(2..9).await.unwrap(), b"llo, Wo");
        assert_eq!(file.read_range(100..).await.unwrap(), b"");
        assert_eq!(file.read_range(0..=4).await.unwrap(), b"Hello");
        assert_eq!(file.read_range(..).await.unwrap(), b"Hello, World!");
    }

    #[tokio::test]
    async fn test_truncate_and_write_params() {
        use crate::{WriteParams, WriteCommandType};
        
        let (_temp_dir, dir) = setup_temp_dir().await;
        let options = GetFileHandleOptions { create: true };

        let mut file = dir
            .get_file_handle_with_options("test.txt", &options)
            .await
            .unwrap();

        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .unwrap();

        // Write initial data
        writer.write_at_cursor_pos(b"Hello, World!".to_vec()).await.unwrap();
        
        // Truncate using WriteParams
        let truncate_params = WriteParams {
            command_type: WriteCommandType::Truncate,
            data: None,
            position: None,
            size: Some(5),
        };
        writer.write_with_params(&truncate_params).await.unwrap();
        
        writer.close().await.unwrap();

        let data = file.read().await.unwrap();
        assert_eq!(data, b"Hello");
    }
}
