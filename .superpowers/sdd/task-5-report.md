# Task 5 报告：Context Menu UI Wiring

## 状态

已完成。

## 实现内容

- 在 `EmailContextMenu.svelte` 中新增“重新加载”菜单项，使用 `lucide-svelte` 的 `RefreshCw` 图标和 `t.email.reload` 文案。
- 为 `EmailContextMenu` 新增 `onReload(emailId)` prop；点击菜单项时调用 `onReload(emailId)`，随后关闭菜单；`disabled` 时沿用 `handleAction` 的早退逻辑，不会执行回调。
- 在 `EmailList.svelte` 中新增 `handleContextReload(emailId)`，调用 `emailState.reloadEmail(emailId)`，并传入 `EmailContextMenu`。
- 在 `zh-CN.ts` 和 `en-US.ts` 的 `email` 文案中新增 `reload`。
- 新增 `src/lib/__tests__/components/EmailContextMenu.test.ts`，覆盖点击“重新加载”调用 `onReload(42)` 并关闭菜单。

## 约束遵守

- 未改动后端、store 逻辑或同步状态逻辑。
- 未触发完整文件夹同步。
- 未实现 batch reload。
- 未添加打开邮件详情时自动 reload 的逻辑。
- 未涉及附件文件磁盘删除逻辑。
- 只修改任务指定文件及本报告文件。

## 验证结果

- `rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/stores/email-state.test.ts src/lib/__tests__/components/EmailContextMenu.test.ts --pool threads --maxWorkers 1 --reporter dot`
  - 结果：失败。
  - 细节：`src/lib/__tests__/stores/email-state.test.ts` 通过 14 个测试；新增组件测试在 render 阶段失败，错误为 Svelte `lifecycle_function_unavailable`: `mount(...) is not available on the server`。
  - 判断：当前 Vitest/Svelte Testing Library 配置未将 `svelte` 解析到 browser/client condition。`@testing-library/svelte` 提供的 Vite 插件可处理该类问题，但全局测试配置不在本任务写入范围内。
- `rtk npx @sveltejs/mcp svelte-autofixer ./src/lib/components/email/EmailContextMenu.svelte --svelte-version 5`
  - 结果：超时/卡住。
  - 细节：运行超过 60 秒无输出，已中断，退出码 130。
- `rtk npm run check`
  - 结果：超时/卡住。
  - 细节：运行超过 90 秒无输出，已中断，退出码 130。
- `rtk node node_modules/typescript/bin/tsc --noEmit --project tsconfig.json`
  - 结果：通过，退出码 0。
- `rtk npm run build`
  - 结果：通过，退出码 0。

## 关注事项

- 新增组件测试的行为断言已按 brief 编写，但当前项目组件测试环境会加载 Svelte server entry，导致无法执行组件 render。未修改 `vitest.config.js`，因为它不在本任务写入范围内。
- Svelte MCP autofixer 和 `npm run check` 在本环境中无输出卡住，已按要求记录。
