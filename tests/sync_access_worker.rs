#![cfg(all(target_arch = "wasm32", feature = "sync-access"))]

use opfs::{DirectoryHandle as _, GetFileHandleOptions};
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

#[wasm_bindgen_test]
async fn sync_file_supports_random_access_and_reopen() {
    let root = opfs::persistent::app_specific_dir()
        .await
        .expect("open OPFS root from dedicated worker");
    let filename = format!("sync-access-test-{}.bin", js_sys::Date::now());
    let file = root
        .get_file_handle_with_options(&filename, &GetFileHandleOptions { create: true })
        .await
        .expect("create test file");

    {
        let sync_file = opfs::sync_access::SyncFile::open(&file)
            .await
            .expect("open sync access handle");
        assert!(sync_file.is_empty().expect("read empty file length"));
        assert_eq!(sync_file.write_at(4, b"OPFS").expect("write at offset"), 4);
        assert_eq!(sync_file.len().expect("read grown file length"), 8);
        sync_file.flush().expect("flush file");
    }

    let sync_file = opfs::sync_access::SyncFile::open(&file)
        .await
        .expect("reopen sync access handle after drop");
    let mut data = [0; 4];
    assert_eq!(sync_file.read_at(4, &mut data).expect("read at offset"), 4);
    assert_eq!(&data, b"OPFS");
    sync_file.set_len(6).expect("truncate file");
    assert_eq!(sync_file.len().expect("read truncated file length"), 6);
    sync_file.close().expect("close file");
}
