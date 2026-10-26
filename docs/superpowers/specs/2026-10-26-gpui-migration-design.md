# Postium Mail GPUI 迁移设计

- 日期：2026-10-26
- 状态：已批准（基于 spike 验证报告）
- 分支：`gpui-migration`

## 目标

将 Postium Mail 的 UI 层从 Tauri 2 + SvelteKit 迁移到 GPUI Kit（纯原生 Rust），
后端逻辑全量复用，功能对齐 v0.2.0，三平台（Windows 优先验证）。

## 非目标

- 不保留 WebView/嵌入网页方案（spike 实测在本机全配置黑屏，已判死刑）
- 不迁移 WebdriverIO E2E 测试框架（GPUI 生态无对应物，后端测试保留）
- 不迁移 MCP 桥（tauri 插件，debug 专用；后续可基于 rmcp 重做）
- 不做自动更新（现状也没有）

## 架构

```
postium-mail/                    # Cargo workspace
├── crates/
│   ├── postium-core/            # 后端：service/domain/infrastructure/error 原样迁入
│   │   └── src/
│   │       ├── domain/          # ← src-tauri/src/domain（零改动）
│   │       ├── service/         # ← src-tauri/src/service（去 tauri 依赖）
│   │       ├── infrastructure/  # ← src-tauri/src/infrastructure（keyring 保留）
│   │       ├── error.rs         # ← src-tauri/src/error
│   │       └── api.rs           # 新：CoreApi 门面，原 tauri command 的进程内等价物
│   ├── postium-editor/          # EmailEditor 组件（自 spike 迁入并产品化）
│   └── postium-ui/              # GPUI 应用（bin）
│       └── src/
│           ├── main.rs          # 应用入口、服务装配、托盘
│           ├── app.rs           # 主窗口壳（侧栏 + 列表 + 详情）
│           ├── stores/          # account/email/sync/theme/toast 状态（自 Svelte stores 移植）
│           ├── views/           # email_list/email_detail/compose/settings/calendar/workflow
│           ├── i18n.rs          # 自 src/lib/i18n 移植
│           ├── tray.rs          # tray-icon crate 集成
│           └── window_state.rs  # 窗口位置/尺寸持久化
├── src-tauri/                   # 过渡期保留，验收后删除
└── src/                         # Svelte 前端，过渡期保留，验收后删除
```

### 关键决策（spike 已验证）

1. **底座**：`gpui-kit = "0.8"` 标准路径。禁用 gpui-fast（与第三方 gpui-pre crate
   类型分裂）、不依赖 webview（无 HWND/DC/opt-level 禁忌）。
2. **写信**：`postium-editor` 封装 `gpui-kit-markdown-editor`（git 依赖，
   crates.io 0.1.0 与 0.8 不兼容）。对外 API：`from_html/set_html/html/is_empty/
   set_link` + `EmailEditorEvent::{Changed, EditLinkRequested}`。
   md⇄HTML 转换产品化：HTML→md 用 html5ever，md→HTML 用 pulldown-cmark + 内联样式后处理。
3. **邮件正文查看**：gpui-kit 原生 `html()` 渲染器。图片由 core 下载/解码后
   以 data URI 注入（内联 cid: 附件必须；远程 https 图默认屏蔽、设置开启）。
   复杂布局邮件提供"在浏览器打开"兜底。已知限制：table 布局保真弱。
4. **IME**：全原生路径，spike 已在微软拼音/微信/Rime 三输入法下验证。
5. **IPC 消失**：UI 直接进程内调用 CoreApi（async service 方法），同步进度事件
   用 smol channel + `cx.spawn` 桥接（spike 已验证模式）。
6. **托盘**：`tray-icon` crate（菜单：显示/写信/同步/退出），事件经 channel 进 GPUI。
7. **对话框**：rfd（保存附件等）。**外部打开**：`open` crate（OAuth 回调页、附件、浏览器兜底）。
8. **OAuth**：复用 core 的 start_oauth2/poll_oauth2，授权页用系统浏览器打开。
9. **i18n/主题**：i18n 移植为 Rust 模块（静态表 + 运行时切换）；主题用 gpui-kit
   内置语义主题（亮/暗切换）。
10. **数据目录**：`~/.postium` 不变，SQLite schema 不变，与 Tauri 版双向兼容。

## 原 command → CoreApi 映射（~40 个）

account：list/get/create/update/delete/update_password
email：list/list_by_category/list_by_category_for_all/get/reload/ensure_attachment_cached/
  save_attachment_as/open_attachment/resolve_inline_attachments/describe_local_attachments/
  search/mark_as_read/toggle_star/delete/move_to_folder/archive/save_draft/delete_draft/
  parse_email_addresses/send
sync：sync_account/sync_account_with_range/get_sync_history_state/sync_older_emails/
  get_folder_stats/get_folder_stats_for_all_accounts/sync_all_accounts
auth：detect_provider/list_providers/start_oauth2/poll_oauth2
label：list/create/update/delete
（窗口命令 open_settings_window 由 UI 层接管）

## 阶段计划（摘要，详单见实施计划文档）

1. Workspace + postium-core 抽取（后端测试全绿）
2. postium-editor 组件迁入产品化
3. UI 骨架：主窗口/侧栏/账号加载/邮件列表（真实数据）
4. 邮件详情：原生渲染 + 图片注入 + 附件操作
5. 写信：EmailEditor + 草稿 + 发送
6. 设置 + OAuth 添加账号
7. 托盘/窗口状态/i18n/主题/日历/工作流
8. 清理 Tauri/Svelte，打包验证

## 风险与缓解

- gpui-kit API 变动（0.8 pre-1.0）→ 锁定版本，升级走单独 PR
- markdown-editor 为单人维护 crate → vendor 进 postium-editor 的可能（fork 备胎）
- table 布局保真 → 浏览器兜底 + 持续跟踪 gpui-kit html 渲染器演进
- GPUI 无障碍/UIA 暴露有限 → 接受（无自动化 E2E 依赖）
