//! 构建脚本：将应用图标嵌入 Windows exe（任务栏/标题栏/资源管理器图标）。

fn main() {
    // 仅 Windows 目标嵌入资源（跨编译安全）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        // exe 属性页的版本信息
        res.set("FileDescription", "Postium Mail");
        res.set("ProductName", "Postium Mail");
        res.set("LegalCopyright", "MIT License");
        res.compile().expect("嵌入 Windows 资源失败");
    }
    println!("cargo:rerun-if-changed=icons/icon.ico");
}
