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
//!
//! ### 组件测试
//! | 测试 | 描述 | 状态 |
//! |------|------|------|
//! | `test_folder_manager_init` | FolderManager 初始化 | ✅ |
//! | `test_change_detector_init` | ChangeDetector 初始化 | ✅ |
//! | `test_mail_processor_init` | MailProcessor 初始化 | ✅ |
//! | `test_sync_state_manager_init` | SyncStateManager 初始化 | ✅ |
//! | `test_delta_sync_init` | DeltaSync 初始化 | ✅ |
//! | `test_sync_components_integration` | 组件集成测试 | ✅ |
//! | `test_imap_auth_info` | ImapAuthInfo 类型测试 | ✅ |
//! | `test_auth_type` | AuthType 枚举测试 | ✅ |
//! | `test_mail_provider_capabilities` | MailProvider 能力测试 | ✅ |
//! | `test_imap_server_config` | ImapServerConfig 测试 | ✅ |
//! | `test_greenmail_config` | GreenMail 配置测试 | ✅ |
//! | `test_sync_strategy` | SyncStrategy 枚举测试 | ✅ |
//! | `test_delta_sync_result` | DeltaSyncResult 测试 | ✅ |
//! | `test_sync_stage` | SyncStage 枚举测试 | ✅ |
//! | `test_sync_progress` | SyncProgress 测试 | ✅ |

mod greenmail_sync_test;
mod sync_flow_test;
mod full_sync_test;
mod test_helpers;
