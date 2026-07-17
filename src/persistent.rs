#[cfg(target_arch = "wasm32")]
pub use crate::web::{DirectoryHandle, FileHandle, WritableFileStream};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::native::{DirectoryHandle, FileHandle, WritableFileStream};

pub type Error = <DirectoryHandle as crate::DirectoryHandle>::Error;
pub type Result<T> = std::result::Result<T, Error>;

/// Returns a directory handle for app-specific data storage.
///
/// On native platforms, this returns the user's data directory (e.g., ~/.local/share on Linux).
/// On web platforms, this returns the root OPFS directory.
#[cfg(target_arch = "wasm32")]
pub async fn app_specific_dir() -> Result<DirectoryHandle> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::FileSystemDirectoryHandle;

    let storage = if let Some(window) = web_sys::window() {
        window.navigator().storage()
    } else {
        js_sys::global()
            .dyn_into::<web_sys::WorkerGlobalScope>()?
            .navigator()
            .storage()
    };

    let root_directory_handle =
        FileSystemDirectoryHandle::from(JsFuture::from(storage.get_directory()).await?);

    Ok(DirectoryHandle::from(root_directory_handle))
}

/// Returns a directory handle for app-specific data storage.
///
/// On native platforms, this returns the user's data directory (e.g., ~/.local/share on Linux).
/// On web platforms, this returns the root OPFS directory.
#[cfg(not(target_arch = "wasm32"))]
pub async fn app_specific_dir() -> Result<DirectoryHandle> {
    let data_dir = dirs::data_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find user data directory",
        )
    })?;

    // Ensure the directory exists
    std::fs::create_dir_all(&data_dir)?;

    Ok(DirectoryHandle::from(data_dir))
}
