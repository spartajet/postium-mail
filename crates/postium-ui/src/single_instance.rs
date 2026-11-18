//! 单实例（线上反馈⑪）：第二个实例启动时唤醒既有实例（显示/重建主窗口）
//! 后自行退出。
//!
//! 通道：本机回环 TCP 握手（跨平台、零新依赖、无 win32 unsafe）——
//! 首个实例绑定 [`PORT`] 成为 Primary 并持有 listener；后续实例 bind 失败
//! → 连接该端口发送 [`MAGIC`]，Primary 回 [`ACK`] 并经事件循环调
//! [`crate::app::show_main_window`]（主窗口存活激活置前 / 已关闭到托盘则
//! 重建）。握手有确认与超时：端口被无关程序占用时不误伤对方也绝不挂死。
//!
//! 检查时机在重量级装配（CoreApi::init）之前——第二实例不碰数据库/引擎。

use std::io::{Read as _, Write as _};



/// 单实例握手端口（本机回环）。避开 OAuth 回调端口 36279。
const PORT: u16 = 45917;
/// 握手魔串（第二实例 → 首实例）。
pub(crate) const MAGIC: &[u8] = b"postium-show\r\n";
/// 确认串（首实例 → 第二实例）。
pub(crate) const ACK: &[u8] = b"postium-ok\r\n";

/// [`acquire`] 的结果。
pub enum Role {
    /// 本实例是首个：持有监听 socket（主窗口打开后交给
    /// [`spawn_show_listener`] 消费）。
    Primary(std::net::TcpListener),
    /// 已有实例在运行（已发送唤醒信号，调用方应立即退出）。
    Secondary,
}

/// 尝试成为 Primary；端口被占（已有实例或无关占用）时执行唤醒握手并返回
/// [`Role::Secondary`]。
pub fn acquire() -> Role {
    match std::net::TcpListener::bind(("127.0.0.1", PORT)) {
        Ok(listener) => Role::Primary(listener),
        Err(_) => {
            let woken = signal_existing();
            tracing::info!(
                target: "postium::ui",
                "单实例：已有实例运行（唤醒握手{}），本实例退出",
                if woken { "成功" } else { "无应答" }
            );
            Role::Secondary
        }
    }
}

/// 连接既有实例发送唤醒信号（带 800ms 超时——绝不挂死启动路径）。
fn signal_existing() -> bool {
    let Ok(mut sock) = std::net::TcpStream::connect(("127.0.0.1", PORT)) else {
        return false;
    };
    let _ = sock.set_read_timeout(Some(std::time::Duration::from_millis(800)));
    let _ = sock.set_write_timeout(Some(std::time::Duration::from_millis(800)));
    if sock.write_all(MAGIC).is_err() {
        return false;
    }
    let mut ack = [0u8; ACK.len()];
    sock.read_exact(&mut ack).is_ok() && &ack == ACK
}
