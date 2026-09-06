# OPFS Rust

Rust wrapper for the the [Origin Private File System](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system) browser API. (This is an API that gives webapps limited access to the native file system.) 

This library mostly exists because using the OPFS from Rust is very painful. As a bonus, it also gives you support for native platforms for free - when compiling to native platforms, it will use `tokio::fs` instead of browser APIs.

## Overview

This crate provides an API for file system operations that automatically uses the appropriate implementation based on the target platform:

- **Web (WASM)**: Uses the Origin Private File System (OPFS) API
- **Native platforms**: Uses `tokio::fs`

An in-memory filesystem is also provided for use in tests (or when persistence isn't necessary)

## Features

- **Write once, run anywhere**: The same code works natively and on the web
- **Async/await**: All operations are asynchronous
- **Type safety**: The type-unsafe JsValue soup associated with working with browser APIs from Rust is hidden behind a type-safe API.

## Installation

```
cargo add opfs
```

## Usage

```rust
use opfs::persistent::{DirectoryHandle, FileHandle, WritableFileStream, app_specific_dir};
use opfs::{GetFileHandleOptions, CreateWritableOptions};
use opfs::persistent;

// you must import the traits to call methods on the types
use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};

// This code works on both native and web platforms
async fn example(dir: DirectoryHandle) -> persistent::Result<()> {
    let options = GetFileHandleOptions { create: true };
    let mut file = dir.get_file_handle_with_options("example.txt", &options).await?;
    
    let write_options = CreateWritableOptions { keep_existing_data: false };
    let mut writer = file.create_writable_with_options(&write_options).await?;
    
    writer.write_at_cursor_pos(b"Hello, world!").await?;
    writer.close().await?;
    
    let data = file.read().await?;
    println!("File contents: {:?}", String::from_utf8(data));
    
    Ok(())
}

async fn use_example() -> persistent::Result<()> {
    // Call once at native startup; browsers already have an origin-specific root.
    #[cfg(not(target_arch = "wasm32"))]
    persistent::configure_app("org", "OPFS", "Example")?;
    let directory: DirectoryHandle = app_specific_dir().await?;
    example(directory).await?;
    Ok(())
}
```

## Native storage configuration

Before opening persistent storage, the native host calls
`persistent::configure_app(qualifier, organization, application)` once. Desktop
platforms use `directories::ProjectDirs::data_local_dir()`; iOS uses Foundation's
Application Support directory inside the app container. Use a stable identity
across app launches. Qualifier and organization may be empty.

For isolated tests or a host-selected location, call
`persistent::configure_root(path)` instead. Relative paths are resolved at
configuration time. Configuration selects the path without creating or moving
files; `app_specific_dir().await` creates it when needed. Missing configuration
and repeated configuration return errors. Existing directory handles can never
be redirected by changing the process configuration.

The browser needs no configuration: `app_specific_dir()` continues to return
the origin's OPFS root. These setup functions are native-only.

Earlier native versions returned the user's shared data directory. Applications
with existing data should select their previous app directory explicitly or
perform their own migration; OPFS does not move existing files.
