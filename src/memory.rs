//! "in-memory" filesystem for use in tests or when persistence isn't necessary

use futures::Stream;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// An entry in a virtual directory in the in-memory filesystem.
pub type DirectoryEntry = crate::DirectoryEntry<DirectoryHandle, FileHandle>;

/// A virtual directory in the in-memory filesystem.
#[derive(Debug, Clone)]
pub struct DirectoryHandle(Rc<RefCell<HashMap<String, DirectoryEntry>>>);

/// A virtual file in the in-memory filesystem.
#[derive(Debug, Clone)]
pub struct FileHandle(WritableFileStream);

/// A writable file stream in the in-memory filesystem.
#[derive(Debug, Clone)]
pub struct WritableFileStream {
    cursor_pos: usize,
    stream: Rc<RefCell<Vec<u8>>>,
}

impl crate::private::Sealed for DirectoryHandle {}
impl crate::private::Sealed for FileHandle {}
impl crate::private::Sealed for WritableFileStream {}

impl crate::DirectoryHandle for DirectoryHandle {
    type Error = String;
    type FileHandleT = FileHandle;

    async fn get_file_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetFileHandleOptions,
    ) -> Result<Self::FileHandleT, Self::Error> {
        let mut directory = self.0.borrow_mut();
        let entry = match directory.entry(name.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                if options.create {
                    let file_handle = FileHandle::new();
                    entry.insert(DirectoryEntry::File(file_handle.clone()));
                    DirectoryEntry::File(file_handle)
                } else {
                    return Err(format!("'{name}' does not exist"));
                }
            }
        };

        match entry {
            DirectoryEntry::Directory(_) => Err(format!("'{name}' is a directory")),
            DirectoryEntry::File(file) => Ok(file),
        }
    }

    async fn get_directory_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetDirectoryHandleOptions,
    ) -> Result<Self, Self::Error> {
        let mut directory = self.0.borrow_mut();
        let entry = match directory.entry(name.to_string()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                if options.create {
                    let dir_handle = DirectoryHandle::default();
                    entry.insert(DirectoryEntry::Directory(dir_handle.clone()));
                    DirectoryEntry::Directory(dir_handle)
                } else {
                    return Err(format!("'{name}' does not exist"));
                }
            }
        };

        match entry {
            DirectoryEntry::File(_) => Err(format!("'{name}' is a file")),
            DirectoryEntry::Directory(dir) => Ok(dir),
        }
    }

    async fn remove_entry(&mut self, name: &str) -> Result<(), Self::Error> {
        let mut directory = self.0.borrow_mut();
        directory.remove(name);
        Ok(())
    }

    async fn remove_entry_with_options(
        &mut self,
        name: &str,
        options: &crate::FileSystemRemoveOptions,
    ) -> Result<(), Self::Error> {
        let mut directory = self.0.borrow_mut();
        
        if let Some(entry) = directory.get(name) {
            match entry {
                DirectoryEntry::Directory(dir) if !options.recursive => {
                    if !dir.0.borrow().is_empty() {
                        return Err(format!("Directory '{}' is not empty", name));
                    }
                }
                _ => {}
            }
        }
        
        directory.remove(name);
        Ok(())
    }

    async fn entries(
        &self,
    ) -> Result<impl Stream<Item = Result<(String, DirectoryEntry), Self::Error>>, Self::Error>
    {
        let directory = self.0.borrow();
        let entries: Vec<_> = directory
            .iter()
            .map(|(name, entry)| Ok((name.clone(), entry.clone())))
            .collect();
        Ok(futures::stream::iter(entries))
    }
}
impl Default for DirectoryHandle {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(HashMap::new())))
    }
}

impl crate::FileHandle for FileHandle {
    type Error = String;
    type WritableFileStreamT = WritableFileStream;

    async fn create_writable_with_options(
        &mut self,
        options: &crate::CreateWritableOptions,
    ) -> Result<Self::WritableFileStreamT, Self::Error> {
        if !options.keep_existing_data {
            self.0.stream.borrow_mut().clear();
        }
        Ok(WritableFileStream {
            cursor_pos: 0,
            ..self.0.clone()
        })
    }

    async fn read(&self) -> Result<Vec<u8>, Self::Error> {
        let stream = self.0.stream.clone();
        let data = stream.borrow().clone();
        Ok(data)
    }

    async fn size(&self) -> Result<usize, Self::Error> {
        Ok(self.0.len())
    }
}

impl crate::WritableFileStream for WritableFileStream {
    type Error = String;

    async fn write_at_cursor_pos(&mut self, data: Vec<u8>) -> Result<(), Self::Error> {
        let data_len = data.len();

        let mut stream = self.stream.borrow_mut();
        *stream = stream[0..self.cursor_pos]
            .iter()
            .cloned()
            .chain(data)
            .collect::<Vec<u8>>();

        self.cursor_pos += data_len;

        Ok(())
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        // no op
        Ok(())
    }

    async fn seek(&mut self, offset: usize) -> Result<(), Self::Error> {
        if offset > self.len() {
            return Err(format!(
                "cannot seek to {offset} because the file is only {len} bytes long",
                len = self.len()
            ));
        }
        self.cursor_pos = offset;
        Ok(())
    }
}

impl FileHandle {
    fn new() -> Self {
        Self(WritableFileStream::new())
    }
}

impl WritableFileStream {
    fn new() -> Self {
        Self {
            cursor_pos: 0,
            stream: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn len(&self) -> usize {
        self.stream.borrow().len()
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

    #[tokio::test]
    async fn test_create_and_read_file() {
        let dir = DirectoryHandle::default();
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
        let dir = DirectoryHandle::default();
        let options = GetFileHandleOptions { create: false };

        let result = dir
            .get_file_handle_with_options("nonexistent.txt", &options)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_remove_entry() {
        let mut dir = DirectoryHandle::default();
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
        let dir = DirectoryHandle::default();
        let entries_stream = dir.entries().await.unwrap();
        let entries: Vec<_> = entries_stream.collect().await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_entries_with_files() {
        let dir = DirectoryHandle::default();
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
    async fn test_seek_and_write() {
        let dir = DirectoryHandle::default();
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
        assert_eq!(data, b"Hi");
    }

    #[tokio::test]
    async fn test_seek_beyond_end() {
        let dir = DirectoryHandle::default();
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

        let result = writer.seek(10).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot seek"));
    }

    #[tokio::test]
    async fn test_keep_existing_data() {
        let dir = DirectoryHandle::default();
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
        writer.close().await.unwrap();

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
        assert_eq!(data, b" World");
    }
}
