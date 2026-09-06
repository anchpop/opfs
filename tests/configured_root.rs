#![cfg(not(target_arch = "wasm32"))]

use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _, persistent};
use std::io::ErrorKind;

// One lifecycle in this test process: configuration is intentionally process-wide.
#[tokio::test]
async fn storage_requires_setup_and_cannot_be_redirected() {
    let error = persistent::app_specific_dir().await.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(error.to_string().contains("configure_app"));
    assert_eq!(
        persistent::configure_root("").unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    for identity in [
        ("", "", " "),
        ("../", "", "Example"),
        ("", "..", "Example"),
        ("", "", "a/b"),
    ] {
        assert_eq!(
            persistent::configure_app(identity.0, identity.1, identity.2)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidInput
        );
    }

    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("nested/app");
    persistent::configure_root(&root).unwrap();
    assert!(!root.exists());
    let directory = persistent::app_specific_dir().await.unwrap();
    assert!(root.is_dir());
    let mut file = directory
        .get_file_handle_with_options("saved.txt", &opfs::GetFileHandleOptions { create: true })
        .await
        .unwrap();
    let mut writer = file
        .create_writable_with_options(&opfs::CreateWritableOptions {
            keep_existing_data: false,
        })
        .await
        .unwrap();
    writer
        .write_at_cursor_pos(b"persistent data")
        .await
        .unwrap();
    writer.close().await.unwrap();
    assert_eq!(
        tokio::fs::read(root.join("saved.txt")).await.unwrap(),
        b"persistent data"
    );

    for path in [&root, &temporary.path().join("other")] {
        assert_eq!(
            persistent::configure_root(path).unwrap_err().kind(),
            ErrorKind::AlreadyExists
        );
    }
    let reopened = persistent::app_specific_dir().await.unwrap();
    assert_eq!(
        reopened
            .get_file_handle_with_options(
                "saved.txt",
                &opfs::GetFileHandleOptions { create: false }
            )
            .await
            .unwrap()
            .read()
            .await
            .unwrap(),
        b"persistent data"
    );
    assert!(!temporary.path().join("other").exists());
}
