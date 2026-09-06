#![cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]

// A separate process from configured_root.rs keeps the process-wide setup isolated.
#[test]
fn application_identity_configures_once_without_creating_storage() {
    opfs::persistent::configure_app("org", "OPFS", "Identity Test").unwrap();
    assert_eq!(
        opfs::persistent::configure_app("org", "OPFS", "Other App")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::AlreadyExists
    );
}
