use anyhow::{Result, anyhow};
use std::{env, path::Path};

// # ========== Google Gmail ==========
// GOOGLE_CLIENT_ID=56071600997-2ggvvrf279h5391a2uka4aigisabbsja.apps.googleusercontent.com
// GOOGLE_CLIENT_SECRET=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
// GOOGLE_REDIRECT_URI=http://localhost:36279/callback

// # ========== Google Workspace ==========
// GOOGLE_WORKSPACE_CLIENT_ID=56071600997-2ggvvrf279h5391a2uka4aigisabbsja.apps.googleusercontent.com
// GOOGLE_WORKSPACE_CLIENT_SECRET=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
// GOOGLE_WORKSPACE_REDIRECT_URI=http://localhost:36279/callback

// # ========== Microsoft Outlook ==========
// MICROSOFT_CLIENT_ID=67acce3b-a85a-40c1-be02-44d954282442
// # Microsoft 公共客户端不需要 client_secret（留空即可）
// MICROSOFT_CLIENT_SECRET=
// MICROSOFT_TENANT=common
// MICROSOFT_REDIRECT_URI=http://localhost:36279/callback

// # ========== Microsoft 365 ==========
// MICROSOFT365_CLIENT_ID=67acce3b-a85a-40c1-be02-44d954282442
// # Microsoft 公共客户端不需要 client_secret（留空即可）
// MICROSOFT365_CLIENT_SECRET=
// MICROSOFT365_TENANT=common
// MICROSOFT365_REDIRECT_URI=http://localhost:36279/callback

fn main() -> Result<()> {
    let env_folder = env::var("CARGO_MANIFEST_DIR")?;
    let env_file_path = Path::new(&env_folder).join(".env");
    if env_file_path.exists() {
        dotenvy::from_path(&env_file_path)?;
    } else {
        return Err(anyhow!("找不到.env 配置文件"));
    }

    let env_items = [
        "GOOGLE_CLIENT_ID",
        "GOOGLE_CLIENT_SECRET",
        "GOOGLE_REDIRECT_URI",
        "GOOGLE_WORKSPACE_CLIENT_ID",
        "GOOGLE_WORKSPACE_CLIENT_SECRET",
        "GOOGLE_WORKSPACE_REDIRECT_URI",
        "MICROSOFT_CLIENT_ID",
        "MICROSOFT_TENANT",
        "MICROSOFT_REDIRECT_URI",
        "MICROSOFT365_CLIENT_ID",
        "MICROSOFT365_TENANT",
        "MICROSOFT365_REDIRECT_URI",
    ];

    for item in env_items {
        if let Ok(value) = env::var(item) {
            println!("cargo:rustc-env={}={}", item, value);
        } else {
            panic!("{} not found in environment", item)
        }
    }
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=build.rs");
    tauri_build::build();
    Ok(())
}
