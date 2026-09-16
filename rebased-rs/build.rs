//! 构建脚本：Windows 目标将应用图标以资源 ID 1 嵌入 exe，
//! gpui 的 Windows 平台层启动时通过 LoadImageW(ID 1) 加载为窗口类与任务栏图标。
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico");
        res.compile().expect("failed to embed windows application icon");
    }
}
