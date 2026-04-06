fn main() {
    let env_folder = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let env_file_path = std::path::Path::new(&env_folder).join(".env");

    if env_file_path.exists() {
        dotenvy::from_path(&env_file_path).expect("Failed to load .env file");
    } else {
        println!("cargo:warning=.env file not found, OAuth2 credentials will not be available");
    }

    // 将 OAuth2 凭据注入编译期环境变量
    let env_items = [
        "GOOGLE_CLIENT_ID",
        "GOOGLE_CLIENT_SECRET",
        "MICROSOFT_CLIENT_ID",
        "MICROSOFT_CLIENT_SECRET",
    ];

    for item in &env_items {
        if let Ok(value) = std::env::var(item) {
            println!("cargo:rustc-env={}={}", item, value);
        }
        // 不 panic，允许缺失（某些 provider 可能不需要所有变量）
    }

    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=build.rs");

    tauri_build::build()
}
