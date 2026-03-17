//! 集成测试模块
//!
//! ## 运行集成测试
//!
//! ### 1. 启动 GreenMail 测试服务器
//!
//! ```bash
//! # 启动服务
//! docker-compose -f docker-compose.test.yml up -d
//!
//! # 查看日志
//! docker logs -f postmium-greenmail
//!
//! # 检查健康状态
//! curl http://localhost:8080/health
//! ```
//!
//! ### 2. 运行集成测试
//!
//! ```bash
//! # 进入 src-tauri 目录
//! cd src-tauri
//!
//! # 运行所有集成测试
//! cargo test --test integration -- --ignored
//!
//! # 运行特定测试
//! cargo test test_greenmail_running -- --ignored
//!
//! # 查看测试输出
//! cargo test --test integration -- --ignored --nocapture
//! ```
//!
//! ### 3. 停止 GreenMail
//!
//! ```bash
//! docker-compose -f docker-compose.test.yml down
//! ```
//!
//! ## 测试列表
//!
//! ### 基础设施测试
//! | 测试 | 描述 | 状态 |
//! |------|------|------|
//! | `test_greenmail_running` | 检查 GreenMail 状态 | ✅ |
//! | `test_greenmail_connection` | 基本 IMAP 连接测试 | ✅ |
//! | `test_docker_compose_file` | 验证 Docker 配置 | ✅ |
//! | `test_tcp_connection_to_greenmail` | TCP 连接和认证测试 | ✅ |
//! | `test_integration_test_structure` | 验证测试文件结构 | ✅ |
//! | `test_documentation_files` | 验证文档完整性 | ✅ |
//!
//! ### 同步功能测试
//! | 测试 | 描述 | 状态 |
//! |------|------|------|
//! | `test_database_initialization` | 数据库初始化 | ✅ |
//! | `test_imap_list_folders` | LIST 命令 | ✅ |
//! | `test_imap_select_inbox` | SELECT 命令 | ✅ |
//! | `test_imap_search_all` | SEARCH 命令 | ✅ |
//! | `test_imap_noop_command` | NOOP 命令 | ✅ |
//! | `test_imap_logout` | LOGOUT 命令 | ✅ |
//! | `test_complete_imap_session` | 完整会话流程 | ✅ |
//! | `test_greenmail_imap_capabilities` | CAPABILITY 命令 | ✅ |

mod greenmail_sync_test;
mod sync_flow_test;
mod test_helpers;
