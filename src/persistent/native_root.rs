use std::{
    io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Configure this process's application identity once, before opening storage.
///
/// Desktop platforms use `directories::ProjectDirs` and its local data directory.
/// Qualifier and organization may be empty; application must not be empty.
/// iOS uses the application's container, which already supplies app isolation.
/// This selects the path; [`super::app_specific_dir`] creates it asynchronously.
pub fn configure_app(qualifier: &str, organization: &str, application: &str) -> io::Result<()> {
    if application.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "application name must not be empty",
        ));
    }
    for component in [qualifier, organization, application] {
        if component.contains(['/', '\\', '\0']) || matches!(component, "." | "..") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "application identity must not contain path components",
            ));
        }
    }
    configure_root(platform_root(qualifier, organization, application)?)
}

/// Configure an explicit native root, for tests or a host-selected storage location.
/// Relative paths are resolved against the working directory at configuration time.
/// Reconfiguration is rejected, even if the new path equals the existing one.
/// No filesystem content is created or moved by this function.
pub fn configure_root(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage root must not be empty",
        ));
    }
    let path = std::path::absolute(path)?;
    ROOT.set(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "native storage root is already configured",
        )
    })
}

pub(super) fn configured_root() -> io::Result<&'static Path> {
    ROOT.get().map(PathBuf::as_path).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "native storage is not configured; call opfs::persistent::configure_app or configure_root at startup")
    })
}

#[cfg(not(target_os = "ios"))]
fn platform_root(qualifier: &str, organization: &str, application: &str) -> io::Result<PathBuf> {
    directories::ProjectDirs::from(qualifier, organization, application)
        .map(|dirs| dirs.data_local_dir().to_owned())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not find application data directory",
            )
        })
}

#[cfg(target_os = "ios")]
fn platform_root(_: &str, _: &str, _: &str) -> io::Result<PathBuf> {
    use objc2_foundation::{
        NSSearchPathDirectory, NSSearchPathDomainMask, NSSearchPathForDirectoriesInDomains,
    };
    let paths = NSSearchPathForDirectoriesInDomains(
        NSSearchPathDirectory::ApplicationSupportDirectory,
        NSSearchPathDomainMask::UserDomainMask,
        true,
    );
    paths
        .firstObject()
        .map(|path| PathBuf::from(path.to_string()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not find application-container support directory",
            )
        })
}

#[cfg(all(test, not(target_os = "ios")))]
mod tests {
    use super::*;

    #[test]
    fn identities_resolve_to_separate_local_app_directories() {
        let root = platform_root("org", "OPFS", "Example").unwrap();
        let other = platform_root("org", "OPFS", "Other").unwrap();
        assert_ne!(root, other);
        assert_eq!(
            root,
            directories::ProjectDirs::from("org", "OPFS", "Example")
                .unwrap()
                .data_local_dir()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn yap_keeps_its_existing_application_support_path() {
        let base = directories::BaseDirs::new().unwrap();
        assert_eq!(
            platform_root("", "", "Yap").unwrap(),
            base.data_dir().join("Yap")
        );
    }
}
