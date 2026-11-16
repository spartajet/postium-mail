//! `--screenshot <file.png>` 支持：Craft Anything 构建检查用。
//!
//! 流程：正常启动 UI（窗口不抢焦点，`focus:false` + `SW_SHOWNOACTIVATE`）→
//! 独立线程等窗口稳定后用 Win32 `PrintWindow(PW_RENDERFULLCONTENT)` 抓客户区 →
//! **先验证画面非空白**（亮度桶数 / 彩色像素占比）才写 PNG；空白则重试，
//! 直到出现真实内容（文字、图标、色块）→ PostMessage(WM_CLOSE) 关窗退出。
//! 不在 render 回调里抓屏：`PrintWindow` 同步等窗口 WndProc，与正在进行的
//! DirectX 帧呈现互等（实测死锁）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// 帧数目标（参考值：真实 UI 在 2 帧内完成首绘，此处不再依赖计数）。
#[allow(dead_code)]
pub const FRAME_TARGET: u32 = 120;
/// 稳定期：窗口尺寸就绪后再等一小段，让 DWM 合成首帧内容。
const SETTLE: Duration = Duration::from_millis(1500);
/// 等待窗口长出真实尺寸的总预算（gpui 首帧布局前客户区是 1×1）。
const WINDOW_WAIT: Duration = Duration::from_secs(10);
/// 单次抓屏重试间隔。
const RETRY_DELAY: Duration = Duration::from_millis(300);
/// 抓屏重试次数（空白画面重抓，直到有内容）。
const RETRY_MAX: u32 = 25;
/// 写盘成功后等待正常退出的宽限期，超时强制退出（保证检查进程收尾）。
const QUIT_GRACE: Duration = Duration::from_secs(3);

static SHOT_PATH: OnceLock<PathBuf> = OnceLock::new();
static FRAMES: AtomicU32 = AtomicU32::new(0);
static DONE: AtomicBool = AtomicBool::new(false);
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// 解析命令行：截图模式启用时登记目标文件并启动抓屏/退出线程。
pub fn init_from_args() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let mut path = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--screenshot" {
            if let Some(v) = args.get(i + 1) {
                path = Some(PathBuf::from(v));
                i += 1;
            }
        }
        i += 1;
    }
    let path = path?;
    let _ = SHOT_PATH.set(path.clone());

    std::thread::Builder::new()
        .name("screenshot".into())
        .spawn(move || run())
        .expect("启动截图线程失败");
    Some(path)
}

fn run() {
    let started = Instant::now();
    // 1) 等主窗口长出真实尺寸（gpui 首帧布局前客户区是 1×1）
    loop {
        if imp::window_ready() {
            break;
        }
        if started.elapsed() > WINDOW_WAIT {
            tracing::warn!("screenshot: 等待窗口超时（客户区未达最小尺寸）");
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // 2) 稳定期：给 DWM 合成首帧内容的时间
    std::thread::sleep(SETTLE);

    if let Some(p) = SHOT_PATH.get() {
        // 重抓直到画面非空白（DWM 合成 / 首帧绘制完成前 PrintWindow 会拿到纯色帧）。
        // capture_window_png 每次都会写文件，最后一次（无论是否空白）保留，
        // 保证进程退出时目标文件一定存在。
        let mut last_err = None;
        for attempt in 0..RETRY_MAX {
            match imp::capture_window_png(p) {
                Ok(true) => {
                    tracing::info!(
                        "screenshot 已写入 {}（第 {} 次尝试，画面非空白）",
                        p.display(),
                        attempt + 1
                    );
                    last_err = None;
                    break;
                }
                Ok(false) => {
                    tracing::debug!(
                        "screenshot: 第 {} 次抓屏为空白，重试",
                        attempt + 1
                    );
                }
                Err(e) => {
                    tracing::warn!("screenshot 抓屏失败（第 {} 次）: {e}", attempt + 1);
                    last_err = Some(e);
                }
            }
            std::thread::sleep(RETRY_DELAY);
        }
        if let Some(e) = last_err {
            tracing::error!("screenshot 失败: {e}");
        }
    }

    DONE.store(true, Ordering::SeqCst);
    QUIT_REQUESTED.store(true, Ordering::SeqCst);
    // 请求关窗（比等下一次 render 的 cx.quit() 更直接；窗口全关 gpui 自行退出）
    imp::request_close();
    // 宽限正常退出，超时强制收尾（PNG 已落盘，数据写入此前均同步完成）
    let deadline = Instant::now() + QUIT_GRACE;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    std::process::exit(0);
}

pub fn enabled() -> bool {
    SHOT_PATH.get().is_some()
}

/// 主循环 render 回调：仅计数；若截图线程已请求退出则退出应用。
pub fn on_frame_tick() {
    if !enabled() {
        return;
    }
    FRAMES.fetch_add(1, Ordering::Relaxed);
}

/// 退出标志（AppShell::render 每帧查询）。
pub fn quit_requested() -> bool {
    QUIT_REQUESTED.load(Ordering::SeqCst)
}

#[cfg(windows)]
mod imp {
    use std::io::Write;
    use std::path::Path;

    const PW_CLIENTONLY: u32 = 0x1;
    const PW_RENDERFULLCONTENT: u32 = 0x2;
    const WM_CLOSE: u32 = 0x0010;
    /// 最小可用客户区（像素²）：gpui 首帧布局完成前窗口是 1×1。
    const MIN_AREA: i64 = 200 * 150;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        bi_size: u32,
        bi_width: i32,
        bi_height: i32,
        bi_planes: u16,
        bi_bit_count: u16,
        bi_compression: u32,
        bi_size_image: u32,
        bi_x_pels_per_meter: i32,
        bi_y_pels_per_meter: i32,
        bi_clr_used: u32,
        bi_clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        bmi_header: BitmapInfoHeader,
        _bmi_colors: [u32; 3],
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
        fn PrintWindow(hwnd: isize, hdc: isize, flags: u32) -> i32;
        fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
        fn EnumWindows(proc: extern "system" fn(isize, usize) -> i32, lparam: usize) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: isize) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(hdc: isize) -> isize;
        fn DeleteDC(hdc: isize) -> i32;
        fn CreateCompatibleBitmap(hdc: isize, w: i32, h: i32) -> isize;
        fn DeleteObject(obj: isize) -> i32;
        fn SelectObject(hdc: isize, obj: isize) -> isize;
        fn GetDIBits(
            hdc: isize,
            bmp: isize,
            start: u32,
            lines: u32,
            bits: *mut u8,
            bmi: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
    }

    /// EnumWindows 回调共享的上下文：本进程可见顶层窗口中客户区最大者。
    #[derive(Default)]
    struct EnumCtx {
        hwnd: isize,
        area: i64,
    }

    unsafe fn find_main_window() -> Option<isize> {
        unsafe {
            let mut ctx = EnumCtx::default();
            extern "system" fn enum_proc(hwnd: isize, lparam: usize) -> i32 {
                unsafe {
                    let ctx = &mut *(lparam as *mut EnumCtx);
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    if pid == std::process::id() && IsWindowVisible(hwnd) != 0 {
                        let mut r = Rect::default();
                        if GetClientRect(hwnd, &mut r) != 0 {
                            let area =
                                (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
                            tracing::debug!(
                                "screenshot: 候选窗口 {hwnd:#x} 客户区 {}x{}",
                                r.right - r.left,
                                r.bottom - r.top
                            );
                            if area > ctx.area {
                                ctx.area = area;
                                ctx.hwnd = hwnd;
                            }
                        }
                    }
                    1 // 继续枚举
                }
            }
            EnumWindows(enum_proc, &mut ctx as *mut EnumCtx as usize);
            // gpui 首帧布局完成前客户区是 1×1（还有托盘辅助窗口）：视为未就绪
            (ctx.area >= MIN_AREA).then_some(ctx.hwnd)
        }
    }

    /// 请求主窗口关闭（截图完成后收尾）。
    pub fn request_close() {
        unsafe {
            if let Some(hwnd) = find_main_window() {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
    }

    /// 主窗口是否已长出真实尺寸（客户区 ≥ MIN_AREA）。
    pub fn window_ready() -> bool {
        unsafe { find_main_window().is_some() }
    }

    /// 抓取主窗口客户区 → PNG（BGRA→RGBA，自上而下行序）。
    /// 返回 Ok(true) 表示画面非空白（有实际内容），Ok(false) 表示纯色/渐变帧。
    pub fn capture_window_png(path: &Path) -> std::io::Result<bool> {
        unsafe {
            let hwnd = find_main_window().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "主窗口未找到")
            })?;

            let mut rect = Rect::default();
            if GetClientRect(hwnd, &mut rect) == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "GetClientRect 失败",
                ));
            }
            let w = (rect.right - rect.left).max(1);
            let h = (rect.bottom - rect.top).max(1);

            let hdc_win = GetDC(hwnd);
            if hdc_win == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "GetDC 失败"));
            }
            let hdc_mem = CreateCompatibleDC(hdc_win);
            let bmp = CreateCompatibleBitmap(hdc_win, w, h);
            let old = SelectObject(hdc_mem, bmp);

            // PW_RENDERFULLCONTENT：DirectX/DWM 渲染窗口（gpui 即此）必需
            let ok = PrintWindow(hwnd, hdc_mem, PW_CLIENTONLY | PW_RENDERFULLCONTENT);
            let mut result = if ok == 0 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "PrintWindow 失败",
                ))
            } else {
                Ok(())
            };

            let mut nonblank = false;
            if result.is_ok() {
                let mut bmi = BitmapInfo {
                    bmi_header: BitmapInfoHeader {
                        bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                        bi_width: w,
                        bi_height: -h, // 负高度 = 自上而下
                        bi_planes: 1,
                        bi_bit_count: 32,
                        bi_compression: 0, // BI_RGB
                        bi_size_image: 0,
                        bi_x_pels_per_meter: 0,
                        bi_y_pels_per_meter: 0,
                        bi_clr_used: 0,
                        bi_clr_important: 0,
                    },
                    _bmi_colors: [0; 3],
                };
                let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
                let got = GetDIBits(
                    hdc_win,
                    bmp,
                    0,
                    h as u32,
                    buf.as_mut_ptr(),
                    &mut bmi,
                    0, // DIB_RGB_COLORS
                );
                if got == 0 {
                    result = Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "GetDIBits 失败",
                    ));
                } else {
                    for px in buf.chunks_exact_mut(4) {
                        px.swap(0, 2); // BGRA → RGBA
                        px[3] = 255; // PrintWindow 无 alpha
                    }
                    nonblank = has_content(&buf);
                    tracing::info!(
                        "screenshot: 抓到 {w}x{h}（{} 字节），nonblank={nonblank}",
                        buf.len()
                    );
                    result = write_png(path, w as u32, h as u32, &buf);
                }
            }

            SelectObject(hdc_mem, old);
            DeleteObject(bmp);
            DeleteDC(hdc_mem);
            ReleaseDC(hwnd, hdc_win);
            result.map(|_| nonblank)
        }
    }

    /// 判断画面是否非空白：亮度直方图（8 级一桶）覆盖 ≥8 桶（文字抗锯齿、
    /// 边缘、阴影都会铺开亮度），或彩色像素（|maxRGB-minRGB|>24）占比 >0.05%
    /// （UI 的彩色图标/链接/徽标）。纯色/背景渐变都过不了这两关。
    fn has_content(rgba: &[u8]) -> bool {
        let mut buckets = [false; 32];
        let mut chroma = 0usize;
        let mut total = 0usize;
        for px in rgba.chunks_exact(4) {
            let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
            let lum = (r * 299 + g * 587 + b * 114) / 1000;
            buckets[(lum / 8) as usize] = true;
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            if mx.saturating_sub(mn) > 24 {
                chroma += 1;
            }
            total += 1;
        }
        let distinct = buckets.iter().filter(|b| **b).count();
        distinct >= 8 || chroma > total / 2000
    }

    fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let f = std::fs::File::create(path)?;
        let mut wtr = std::io::BufWriter::new(f);
        let mut enc = png::Encoder::new(&mut wtr, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc
            .write_header()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        drop(writer);
        wtr.flush()?;
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;
    /// 非 Windows：写 1×1 占位 PNG（平台截屏需按 OS 补实现）。
    pub fn capture_window_png(path: &Path) -> std::io::Result<bool> {
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, 1, 1);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(io_err)?;
            w.write_image_data(&[0, 0, 0, 255]).map_err(io_err)?;
        }
        std::fs::write(path, buf)?;
        Ok(false)
    }
    /// 非 Windows：无窗口可关，交由 quit_requested 路径退出。
    pub fn request_close() {}
    /// 非 Windows：无 Win32 窗口概念，直接视为就绪。
    pub fn window_ready() -> bool {
        true
    }
    fn io_err(e: impl std::fmt::Display) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    }
}
