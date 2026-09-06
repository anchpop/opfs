#[cfg(target_arch = "wasm32")]
pub use crate::web::{DirectoryHandle, FileHandle, WritableFileStream};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::native::{DirectoryHandle, FileHandle, WritableFileStream};

#[cfg(not(target_arch = "wasm32"))]
mod native_root;
#[cfg(not(target_arch = "wasm32"))]
pub use native_root::{configure_app, configure_root};

pub type Error = <DirectoryHandle as crate::DirectoryHandle>::Error;
pub type Result<T> = std::result::Result<T, Error>;

/// Returns a directory handle for app-specific data storage.
///
/// On web platforms, this returns the origin's root OPFS directory. No setup is needed.
#[cfg(target_arch = "wasm32")]
pub async fn app_specific_dir() -> Result<DirectoryHandle> {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::FileSystemDirectoryHandle;

    let window = web_sys::window().ok_or("No window object")?;
    let navigator = window.navigator();

    let root_directory_handle =
        FileSystemDirectoryHandle::from(JsFuture::from(navigator.storage().get_directory()).await?);

    Ok(DirectoryHandle::from(root_directory_handle))
}

/// Returns a directory handle for app-specific data storage.
///
/// Configure native storage once with [`configure_app`] or [`configure_root`]
/// before calling this function. Creates the configured directory if necessary.
/// Returns an error when storage has not been configured; never falls back to
/// the user's shared data directory.
#[cfg(not(target_arch = "wasm32"))]
pub async fn app_specific_dir() -> Result<DirectoryHandle> {
    let root = native_root::configured_root()?;
    tokio::fs::create_dir_all(root).await?;
    Ok(DirectoryHandle::from(root.to_owned()))
}
