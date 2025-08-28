use futures::Stream;
use futures::StreamExt;
use js_sys::{ArrayBuffer, AsyncIterator, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, stream::JsStream};
use web_sys::{
    FileSystemCreateWritableOptions, FileSystemDirectoryHandle, FileSystemFileHandle,
    FileSystemGetFileOptions, FileSystemRemoveOptions, FileSystemWritableFileStream,
};

type DirectoryEntry = crate::DirectoryEntry<DirectoryHandle, FileHandle>;

#[derive(Debug, Clone)]
pub struct DirectoryHandle(FileSystemDirectoryHandle);

#[derive(Debug, Clone)]
pub struct FileHandle(FileSystemFileHandle);

#[derive(Debug, Clone)]
pub struct WritableFileStream(FileSystemWritableFileStream);

#[derive(Debug, Clone)]
pub struct File(web_sys::File);

impl From<FileSystemDirectoryHandle> for DirectoryHandle {
    fn from(handle: FileSystemDirectoryHandle) -> Self {
        Self(handle)
    }
}

impl From<FileSystemFileHandle> for FileHandle {
    fn from(handle: FileSystemFileHandle) -> Self {
        Self(handle)
    }
}

impl From<FileSystemWritableFileStream> for WritableFileStream {
    fn from(handle: FileSystemWritableFileStream) -> Self {
        Self(handle)
    }
}

impl From<web_sys::File> for File {
    fn from(handle: web_sys::File) -> Self {
        Self(handle)
    }
}

impl crate::private::Sealed for DirectoryHandle {}
impl crate::private::Sealed for FileHandle {}
impl crate::private::Sealed for WritableFileStream {}

impl crate::DirectoryHandle for DirectoryHandle {
    type Error = JsValue;
    type FileHandleT = FileHandle;

    async fn get_file_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetFileHandleOptions,
    ) -> Result<Self::FileHandleT, Self::Error> {
        let fs_options = FileSystemGetFileOptions::new();
        fs_options.set_create(options.create);
        let file_system_file_handle = FileSystemFileHandle::from(
            JsFuture::from(self.0.get_file_handle_with_options(name, &fs_options)).await?,
        );
        Ok(FileHandle(file_system_file_handle))
    }

    async fn get_directory_handle_with_options(
        &self,
        name: &str,
        options: &crate::GetDirectoryHandleOptions,
    ) -> Result<Self, Self::Error> {
        use web_sys::FileSystemGetDirectoryOptions;

        let fs_options = FileSystemGetDirectoryOptions::new();
        fs_options.set_create(options.create);
        let file_system_directory_handle = FileSystemDirectoryHandle::from(
            JsFuture::from(self.0.get_directory_handle_with_options(name, &fs_options)).await?,
        );
        Ok(DirectoryHandle(file_system_directory_handle))
    }

    async fn remove_entry(&mut self, name: &str) -> Result<(), Self::Error> {
        JsFuture::from(self.0.remove_entry(name)).await?;
        Ok(())
    }

    async fn remove_entry_with_options(
        &mut self,
        name: &str,
        options: &crate::FileSystemRemoveOptions,
    ) -> Result<(), Self::Error> {
        let fs_options = FileSystemRemoveOptions::new();
        fs_options.set_recursive(options.recursive);
        JsFuture::from(self.0.remove_entry_with_options(name, &fs_options)).await?;
        Ok(())
    }

    async fn entries(
        &self,
    ) -> Result<impl Stream<Item = Result<(String, DirectoryEntry), Self::Error>>, Self::Error>
    {
        let entries_iterator = self.0.entries();
        let async_iterator = AsyncIterator::from(entries_iterator);
        let js_stream: JsStream = JsStream::from(async_iterator);

        let stream = js_stream.map(|item| {
            match item {
                Ok(js_array) => {
                    // entries() returns [key, value] pairs
                    let array = js_sys::Array::from(&js_array);
                    let filename = array
                        .get(0)
                        .as_string()
                        .ok_or_else(|| JsValue::from_str("Invalid filename"))?;
                    let handle = array.get(1);

                    // Determine if it's a file or directory handle
                    let entry = if handle.has_type::<FileSystemFileHandle>() {
                        DirectoryEntry::File(FileHandle(FileSystemFileHandle::from(handle)))
                    } else if handle.has_type::<FileSystemDirectoryHandle>() {
                        DirectoryEntry::Directory(DirectoryHandle(FileSystemDirectoryHandle::from(
                            handle,
                        )))
                    } else {
                        return Err(JsValue::from_str("Unknown handle type"));
                    };

                    Ok((filename, entry))
                }
                Err(e) => Err(e),
            }
        });

        Ok(stream)
    }
}

impl crate::FileHandle for FileHandle {
    type Error = JsValue;
    type WritableFileStreamT = WritableFileStream;

    async fn create_writable_with_options(
        &mut self,
        options: &crate::CreateWritableOptions,
    ) -> Result<Self::WritableFileStreamT, Self::Error> {
        let fs_options = FileSystemCreateWritableOptions::new();
        fs_options.set_keep_existing_data(options.keep_existing_data);
        let file_system_writable_file_stream = FileSystemWritableFileStream::unchecked_from_js(
            JsFuture::from(self.0.create_writable_with_options(&fs_options)).await?,
        );
        Ok(WritableFileStream(file_system_writable_file_stream))
    }

    async fn read(&self) -> Result<Vec<u8>, Self::Error> {
        self.get_file().await?.read().await
    }

    async fn size(&self) -> Result<usize, Self::Error> {
        let size = self.get_file().await?.size();
        Ok(size)
    }
}

impl FileHandle {
    pub async fn get_file(&self) -> Result<File, JsValue> {
        let file: web_sys::File = JsFuture::from(self.0.get_file()).await?.into();
        Ok(File(file))
    }
}

impl crate::WritableFileStream for WritableFileStream {
    type Error = JsValue;

    async fn write_at_cursor_pos(&mut self, data: Vec<u8>) -> Result<(), Self::Error> {
        // You'd think we could just do
        // ```
        // JsFuture::from(self.0.write_with_u8_array(data.as_mut_slice())?).await?;
        // ```
        // But a safari bug makes this write basically the entire wasm heap to the file.
        // So we have to write as a File first.

        let uint8_array = js_sys::Uint8Array::from(data.as_slice());
        let array = js_sys::Array::new();
        array.push(&uint8_array);
        let File = web_sys::File::new_with_u8_array_sequence(&array)?;

        JsFuture::from(self.0.write_with_File(&File)?).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        JsFuture::from(self.0.close()).await?;
        Ok(())
    }

    async fn seek(&mut self, offset: usize) -> Result<(), Self::Error> {
        JsFuture::from(self.0.seek_with_u32(offset as u32)?).await?;
        Ok(())
    }
}

impl File {
    fn size(&self) -> usize {
        self.0.size() as usize
    }

    async fn read(&self) -> Result<Vec<u8>, JsValue> {
        let buffer = ArrayBuffer::unchecked_from_js(JsFuture::from(self.0.array_buffer()).await?);
        let uint8_array = Uint8Array::new(&buffer);
        let mut vec = vec![0; self.size()];
        uint8_array.copy_to(&mut vec);
        Ok(vec)
    }

    #[allow(dead_code)]
    pub async fn text(&self) -> Result<String, JsValue> {
        JsFuture::from(self.0.text())
            .await?
            .as_string()
            .ok_or(JsValue::NULL)
    }
}
