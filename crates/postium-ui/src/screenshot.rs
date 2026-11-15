//! `--screenshot <file.png>` 支持：Craft Anything 构建检查用。
//!
//! 流程：正常启动 UI（窗口不抢焦点，`focus:false` + `SW_SHOWNOACTIVATE`）→
//! 渲染帧计数到目标（120）→ **独立线程**用 Win32 `PrintWindow(PW_RENDERFULLCONTENT)`
//! 抓客户区 → PNG 落盘 → 置退出标志，下一帧主循环 `cx.quit()`。
//! 不在 render 回调里抓屏：`PrintWindow` 会同步等窗口 WndProc，与正在进行的
//! DirectX 帧呈现互等（实测死锁）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const FRAME_TARGET: u32 = 120;
/// 稳定期：窗口绘制稳定后再抓屏（实测 <1s 内管线就绪；未聚焦时 gpui 不再
/// 主动产帧，帧计数不可靠，故用固定稳定期 + 帧计数上限双条件）。
const SETTLE: Duration = Duration::from_secs(2);
/// 截图线程兜底超时。
const WAIT_TIMEOUT: Duration = Duration::from_secs(30);

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

    // 抓屏线程：等帧到位（或超时）→ 抓 PNG → 请求退出
    std::thread::Builder::new()
        .name("screenshot".into())
        .spawn(move || {
            let started = Instant::now();
            // 双条件：帧数达标 或 稳定期+帧≥2（未聚焦时 gpui 事件驱动，帧计数停滞）
            loop {
                let frames = FRAMES.load(Ordering::Relaxed);
                if frames >= FRAME_TARGET {
                    break;
                }
                if started.elapsed() >= SETTLE && frames >= 2 {
                    tracing::debug!("screenshot: 稳定期已过（{frames} 帧），开始抓屏");
                    break;
                }
                if started.elapsed() > WAIT_TIMEOUT {
                    tracing::warn!(
                        "screenshot: 帧计数未达目标（{frames}/{}），按超时抓屏",
                        FRAME_TARGET
                    );
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            if let Some(p) = SHOT_PATH.get() {
                // 重试抓屏直到出现非空内容（DWM 合成到位前 PrintWindow 可能拿到空白帧）
                let mut last_err = None;
                for attempt in 0..8 {
                    match capture_window_png(p) {
                        Ok(()) => {
                            let nontrivial = std::fs::metadata(p)
                                .map(|m| m.len() > 5_000)
                                .unwrap_or(false);
                            if nontrivial || attempt == 7 {
                                tracing::info!("screenshot 已写入 {}（第 {} 次尝试）", p.display(), attempt + 1);
                                break;
                            }
                        }
                        Err(e) => last_err = Some(e),
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                if let Some(e) = last_err {
                    tracing::error!("screenshot 失败: {e}");
                }
            }
            DONE.store(true, Ordering::SeqCst);
            QUIT_REQUESTED.store(true, Ordering::SeqCst);
        })
        .expect("启动截图线程失败");
    Some(path)
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
        fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
        fn PrintWindow(hwnd: isize, hdc: isize, flags: u32) -> i32;
        fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
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

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn find_main_window() -> Option<isize> {
        unsafe {
            let title = wide("Postium Mail");
            let mut hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if hwnd == 0 {
                // 标题可能带邮件主题前缀：退而求其次按常见 gpui/Win32 类名尝试
                for cls in ["Chrome_WidgetWin_0", "GLFW30", "postium-mail"] {
                    let c = wide(cls);
                    hwnd = FindWindowW(c.as_ptr(), std::ptr::null());
                    if hwnd != 0 {
                        break;
                    }
                }
            }
            (hwnd != 0).then_some(hwnd)
        }
    }

    /// 抓取主窗口客户区 → PNG（BGRA→RGBA，自上而下行序）。
    pub fn capture_window_png(path: &Path) -> std::io::Result<()> {
        unsafe {
            let hwnd = find_main_window().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "主窗口未找到")
            })?;
            // Windows：压到 Z 序底层，避免挡住用户（覆盖窗口依旧绘制）
            const SWP_NOSIZE: u32 = 0x1;
            const SWP_NOMOVE: u32 = 0x2;
            const SWP_NOACTIVATE: u32 = 0x10;
            SetWindowPos(
                hwnd,
                1, // HWND_BOTTOM
                0,
                0,
                0,
                0,
                SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE,
            );

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
                    result = write_png(path, w as u32, h as u32, &buf);
                }
            }

            SelectObject(hdc_mem, old);
            DeleteObject(bmp);
            DeleteDC(hdc_mem);
            ReleaseDC(hwnd, hdc_win);
            result
        }
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

#[cfg(windows)]
use imp::capture_window_png;

#[cfg(not(windows))]
mod imp {
    use std::path::Path;
    /// 非 Windows：写 1×1 占位 PNG（平台截屏需按 OS 补实现）。
    pub fn capture_window_png(path: &Path) -> std::io::Result<()> {
        let mut buf = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut buf, 1, 1);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(io_err)?;
            w.write_image_data(&[0, 0, 0, 255]).map_err(io_err)?;
        }
        std::fs::write(path, buf)
    }
    fn io_err(e: impl std::fmt::Display) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    }
}

#[cfg(not(windows))]
use imp::capture_window_png;
