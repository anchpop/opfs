#![cfg(all(target_arch = "wasm32", feature = "redb"))]

use opfs::{DirectoryHandle as _, GetFileHandleOptions};
use redb::{ReadableDatabase, TableDefinition};
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_dedicated_worker);

const VALUES: TableDefinition<&str, &str> = TableDefinition::new("values");

#[wasm_bindgen_test]
async fn redb_round_trips_through_opfs() {
    let root = opfs::persistent::app_specific_dir()
        .await
        .expect("open OPFS root from dedicated worker");
    let filename = format!("redb-opfs-test-{}.redb", js_sys::Date::now());
    let file = root
        .get_file_handle_with_options(&filename, &GetFileHandleOptions { create: true })
        .await
        .expect("create database file");

    {
        let backend = opfs::redb::OpfsBackend::open(&file)
            .await
            .expect("open sync access handle");
        let database = redb::Database::builder()
            .create_with_backend(backend)
            .expect("create redb database");
        let transaction = database.begin_write().expect("begin write");
        {
            let mut table = transaction.open_table(VALUES).expect("open table");
            table.insert("answer", "42").expect("insert value");
        }
        transaction.commit().expect("commit value");
    }

    {
        let backend = opfs::redb::OpfsBackend::open(&file)
            .await
            .expect("reopen sync access handle");
        let database = redb::Database::builder()
            .create_with_backend(backend)
            .expect("reopen redb database");
        let transaction = database.begin_read().expect("begin read");
        let table = transaction.open_table(VALUES).expect("reopen table");
        let value = table
            .get("answer")
            .expect("read value")
            .expect("stored value exists");
        assert_eq!(value.value(), "42");
    }
}
