状态：DONE

修改文件列表：
- src-tauri/tests/account_commands.rs
- src-tauri/src/infrastructure/storage/repository/label_repo.rs
- src-tauri/src/infrastructure/storage/repository/email_repo.rs
- src-tauri/src/infrastructure/storage/repository/sync_repo.rs
- src-tauri/src/service/account_service.rs

红灯测试命令和失败摘要：
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_removes_local_account_data_without_foreign_keys --test account_commands -- --nocapture`
- 结果：失败，退出码 101。
- 摘要：新增测试 `test_delete_account_removes_local_account_data_without_foreign_keys` 失败。失败断言在 `tests/account_commands.rs:345:5`，`left: 1`、`right: 0`，说明删除账号后账号 A 的本地数据仍然存在，符合预期红灯。

绿灯测试命令和通过摘要：
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_removes_local_account_data_without_foreign_keys --test account_commands -- --nocapture`
- 结果：通过，`cargo test: 1 passed, 8 filtered out (1 suite, 0.03s)`。
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands`
- 结果：通过，`cargo test: 9 passed (1 suite, 0.16s)`。

rustfmt 命令和结果：
- 命令：`rtk rustfmt --edition 2024 src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs`
- 结果：通过，退出码 0。
- 命令：`rtk rustfmt --edition 2024 --check src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs`
- 结果：通过，退出码 0。

提交 hash：
- 33355639e5e0e7cd557ee31d87931607f3e49eeb

自审结论和 concerns：
- 自审结论：实现限定在 brief 指定的 5 个 Rust 文件中。账号删除现在显式清理账号下的 email_labels、labels、attachments、emails、sync_errors、sync_state，再删除账号记录和 keyring 密码，不依赖 SQLite foreign keys；未修改迁移、FTS、软删除或无关业务。
- Concerns：无。

---

状态：DONE（reviewer Important fixes）

修改文件列表：
- src-tauri/src/infrastructure/storage/repository/account_repo.rs
- src-tauri/src/infrastructure/storage/repository/email_repo.rs
- src-tauri/src/infrastructure/storage/repository/label_repo.rs
- src-tauri/src/infrastructure/storage/repository/sync_repo.rs
- src-tauri/src/service/account_service.rs
- src-tauri/tests/account_commands.rs

修复说明：
- 修复 `AccountService::delete` 多表删除缺少事务保护：账号存在性检查后开启 SeaORM transaction，`email_labels`、`labels`、`attachments`、`emails`、`sync_errors`、`sync_state` 和账号行删除全部在同一事务中执行，commit 成功后才执行 keyring 删除；keyring 删除仍忽略错误且不阻断。
- 修复 `email_repo::delete_by_account` / `label_repo::delete_by_account` 的 SQLite bind 参数扩展风险：移除先加载全部 `email_ids` / `label_ids` 再 `is_in(...)` 的实现，改为 `DELETE ... WHERE ... IN (SELECT id FROM ... WHERE account_id = ?)` raw SQL，每条语句只绑定 `account_id`。
- 新增事务回滚测试 `test_delete_account_rolls_back_local_data_when_account_row_delete_fails`：通过 trigger 强制账号行删除失败，先确认无事务实现下红灯，随后验证关联数据和 keyring 都保留。

红灯测试命令和失败摘要：
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_rolls_back_local_data_when_account_row_delete_fails --test account_commands -- --nocapture`
- 结果：失败，退出码 101。
- 摘要：新增回滚测试失败在 `tests/account_commands.rs:536:5`，邮件计数 `left: 0`、`right: 1`，说明账号行删除失败后前置关联数据删除未回滚，符合预期红灯。

测试命令和结果：
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_rolls_back_local_data_when_account_row_delete_fails --test account_commands -- --nocapture`
- 结果：通过，`cargo test: 1 passed, 9 filtered out (1 suite, 0.04s)`。
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml test_delete_account_removes_local_account_data_without_foreign_keys --test account_commands -- --nocapture`
- 结果：通过，`cargo test: 1 passed, 9 filtered out (1 suite, 0.03s)`。
- 命令：`rtk cargo test --manifest-path src-tauri/Cargo.toml --test account_commands`
- 结果：通过，`cargo test: 10 passed (1 suite, 0.19s)`。

rustfmt 命令和结果：
- 命令：`rtk rustfmt --edition 2024 --check src-tauri/tests/account_commands.rs src-tauri/src/infrastructure/storage/repository/account_repo.rs src-tauri/src/infrastructure/storage/repository/label_repo.rs src-tauri/src/infrastructure/storage/repository/email_repo.rs src-tauri/src/infrastructure/storage/repository/sync_repo.rs src-tauri/src/service/account_service.rs`
- 结果：通过，退出码 0。

提交 hash：
- 代码修复提交：385127a1b1016838c8240147be83aee40eb0b04a

Concerns：
- 无。
