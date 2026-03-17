//! 流程引擎
//!
//! 协调整个邮件客户端的工作流程

use crate::error::Result;

/// 流程引擎
pub struct FlowEngine {
    // TODO: 实现流程引擎逻辑
}

impl FlowEngine {
    pub fn new() -> Self {
        Self {}
    }

    /// 启动引擎
    pub async fn start(&self) -> Result<()> {
        // TODO: 实现引擎启动
        tracing::info!("流程引擎启动");
        Ok(())
    }

    /// 停止引擎
    pub async fn stop(&self) -> Result<()> {
        // TODO: 实现引擎停止
        tracing::info!("流程引擎停止");
        Ok(())
    }
}

impl Default for FlowEngine {
    fn default() -> Self {
        Self::new()
    }
}
