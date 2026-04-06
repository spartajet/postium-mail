# Phase 6: 集成测试 + 清理

> 前置：Phase 2 + Phase 4 + Phase 5 完成
> 完成标志：系统托盘可用，错误处理统一，性能基线达标，全局 lint 通过

---

### Task 6.1: 系统托盘实现

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml` (添加 tauri-plugin-shell 或确认 tray 依赖)
- Modify: `src-tauri/tauri.conf.json`

**Step 1: 配置 tauri.conf.json 允许托盘**

在 `tauri.conf.json` 的 `app` 下添加 trayIcon 配置：

```json
"trayIcon": {
  "iconPath": "icons/icon.png",
  "iconAsTemplate": true,
  "id": "main-tray"
}
```

**Step 2: 在 lib.rs 中创建系统托盘**

在 `run()` 函数中 `tauri::Builder` 之前添加托盘菜单：

```rust
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;

let tray = TrayIconBuilder::with_id("main-tray")
    .menu(&MenuBuilder::new(app)
        .items(&[
            &MenuItemBuilder::with_id("show", "显示窗口").build(app)?,
            &MenuItemBuilder::with_id("new_email", "写邮件").build(app)?,
            &MenuItemBuilder::with_id("sync", "同步").build(app)?,
            &MenuItemBuilder::with_id("quit", "退出").build(app)?,
        ])
        .build()?
    )
    .on_menu_event(move |app, event| {
        match event.id().as_ref() {
            "show" => {
                let _ = app.get_webview_window("main").unwrap().show();
                let _ = app.get_webview_window("main").unwrap().set_focus();
            }
            "new_email" => {
                let _ = app.emit("tray-action", "compose");
            }
            "sync" => {
                let _ = app.emit("tray-action", "sync");
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        }
    })
    .build(app)?;
```

**Step 3: 在前端监听托盘事件**

在 `+layout.svelte` 添加：

```svelte
<script lang="ts">
  import { listen } from '@tauri-apps/api/event';

  onMount(() => {
    const unlisten = listen<string>('tray-action', (event) => {
      if (event.payload === 'compose') {
        // 触发写邮件
      } else if (event.payload === 'sync') {
        // 触发同步
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  });
</script>
```

**Step 4: 验证**

Run: `bun run tauri dev`
Expected: 系统托盘显示图标，右键菜单可用，点击"显示窗口"可聚焦

**Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/tauri.conf.json src/routes/+layout.svelte
git commit -m "feat: implement system tray with context menu"
```

---

### Task 6.2: 关闭到托盘 + 窗口隐藏

**Files:**
- Modify: `src/lib/components/layout/TitleBar.svelte`

**Step 1: 修改关闭按钮行为**

将关闭按钮改为隐藏窗口而不是退出：

```svelte
<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const appWindow = getCurrentWindow();

  async function minimize() {
    await appWindow.minimize();
  }

  async function toggleMaximize() {
    await appWindow.toggleMaximize();
  }

  async function hideToTray() {
    await appWindow.hide();
  }
</script>

<!-- 关闭按钮改为 hideToTray -->
<button
  class="inline-flex h-full w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
  onclick={hideToTray}
  aria-label="关闭到托盘"
>
  <!-- ... SVG unchanged ... -->
</button>
```

**Step 2: 在 tauri.conf.json 配置关闭行为**

确保 `windows` 配置中没有 `"visibleOnAllWorkspaces"` 等冲突项。

**Step 3: 验证**

Run: `bun run tauri dev`
Expected: 点击关闭按钮后窗口隐藏到托盘，托盘点击"显示窗口"可恢复

**Step 4: Commit**

```bash
git add src/lib/components/layout/TitleBar.svelte
git commit -m "feat: close button hides window to system tray"
```

---

### Task 6.3: 全局错误处理统一

**Files:**
- Create: `src/lib/utils/error.ts`
- Modify: `src/routes/+layout.svelte`

**Step 1: 写错误处理工具**

`src/lib/utils/error.ts`:

```typescript
import { MailError } from '$lib/bindings';

/**
 * 将 Tauri Command 错误转换为用户友好消息
 */
export function formatError(error: unknown): string {
  if (error instanceof Error) {
    const msg = error.message;

    // 尝试解析 MailError JSON
    try {
      const parsed = JSON.parse(msg);
      if (parsed.type && parsed.message) {
        return parsed.message;
      }
    } catch {}

    return msg;
  }

  if (typeof error === 'string') {
    return error;
  }

  return '未知错误';
}

/**
 * 全局未处理错误提示
 */
export function setupGlobalErrorHandler() {
  window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
    // 可在此接入 Toast 通知系统
  });
}
```

**Step 2: 在 layout 中初始化**

在 `+layout.svelte` 的 `onMount` 中调用：

```typescript
import { setupGlobalErrorHandler } from '$lib/utils/error';

onMount(() => {
  setupGlobalErrorHandler();
});
```

**Step 3: 验证**

Run: `bun run dev`
Expected: 编译通过，控制台无 TypeScript 错误

**Step 4: Commit**

```bash
git add src/lib/utils/error.ts src/routes/+layout.svelte
git commit -m "feat: add global error handler with user-friendly messages"
```

---

### Task 6.4: 前端 Lint + TypeScript 严格检查

**Files:**
- Modify: `svelte.config.js` (如果需要调整)
- Modify: `tsconfig.json`

**Step 1: 确保 TypeScript 严格模式**

检查 `tsconfig.json`：

```json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true
  }
}
```

**Step 2: 运行 svelte-check**

Run: `bun run svelte-check --threshold error`

**Step 3: 修复所有错误**

根据输出逐一修复 TypeScript / Svelte 类型错误。

常见修复模式：
- `invoke<T>` 需要导入绑定类型
- `$props()` 需要显式类型注解
- `@html` 需要注意安全

**Step 4: Commit**

```bash
git add -A
git commit -m "fix: resolve all TypeScript strict mode errors"
```

---

### Task 6.5: CSS 清理 + 主题一致性检查

**Files:**
- Modify: `src/app.css`
- 检查所有组件的 class 一致性

**Step 1: 检查 CSS 变量完整**

在 `app.css` 的 `@theme` 中确认以下变量全部定义：

- `--background`, `--foreground`
- `--card`, `--card-foreground`
- `--primary`, `--primary-foreground`
- `--muted`, `--muted-foreground`
- `--border`, `--input`, `--ring`
- `--destructive`, `--destructive-foreground`

在 `.dark` 选择器中确认暗色对应值。

**Step 2: 检查组件 class 一致性**

运行全局搜索确保：
- 所有 `border-border` 一致使用
- 所有 `text-muted-foreground` 一致使用
- 所有 `bg-primary` / `text-primary-foreground` 一致使用
- 无硬编码颜色值（除 provider 配置外）

**Step 3: 验证主题切换**

Run: `bun run tauri dev`
- 浅色模式：所有文本可读，边框可见
- 深色模式：所有文本可读，边框可见，毛玻璃效果正常

**Step 4: Commit**

```bash
git add src/app.css
git commit -m "fix: ensure theme consistency across light/dark modes"
```

---

### Task 6.6: 后端编译 + Clippy 检查

**Files:**
- 修复所有 Clippy 警告

**Step 1: 运行 cargo clippy**

Run: `cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings`

**Step 2: 修复所有警告**

常见修复：
- 移除未使用的导入
- 添加 `#[allow(dead_code)]` 给预留方法
- 修复 `clone()` 过度使用
- 确保 `async` 函数没有不必要的 `await`

**Step 3: 验证 cargo check**

Run: `cd src-tauri && cargo check`
Expected: 无 warning 无 error

**Step 4: Commit**

```bash
git add -A
git commit -m "fix: resolve all Rust clippy warnings"
```

---

### Task 6.7: 性能基线检查

**Files:**
- 无新文件，检查现有代码

**Step 1: 检查打包大小**

Run: `bun run build`
检查 `build/` 目录大小，确保：
- JS bundle < 500KB (gzipped < 150KB)
- CSS bundle < 100KB
- 无多余大型依赖

**Step 2: 检查 Rust 二进制大小**

Run: `cd src-tauri && cargo build --release`
检查 `target/release/` 中可执行文件大小。

如果过大，在 `Cargo.toml` 添加优化配置：

```toml
[profile.release]
opt-level = "s"
lto = true
strip = true
codegen-units = 1
```

**Step 3: 检查启动时间**

Run: `bun run tauri dev`
- 冷启动到 UI 渲染 < 3 秒
- 文件夹切换响应 < 500ms

**Step 4: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "perf: configure release optimizations for smaller binary"
```

---

### Task 6.8: 路由集成检查

**Files:**
- Modify: `src/lib/components/layout/Sidebar.svelte` (添加路由导航)

**Step 1: 在 Sidebar 添加导航链接**

确保日历、工作流、设置按钮使用 SvelteKit 路由导航：

```svelte
<script lang="ts">
  import { goto } from '$app/navigation';
</script>

<!-- 日历按钮 -->
<button onclick={() => goto('/calendar')}>...</button>

<!-- 工作流按钮 -->
<button onclick={() => goto('/workflow')}>...</button>

<!-- 设置按钮 -->
<button onclick={() => goto('/settings')}>...</button>
```

**Step 2: 验证所有路由可访问**

Run: `bun run dev`
- `/` — 三栏邮件布局
- `/calendar` — 日历视图
- `/workflow` — 工作流编辑器
- `/settings` — 设置页面

**Step 3: Commit**

```bash
git add src/lib/components/layout/Sidebar.svelte
git commit -m "feat: connect sidebar navigation to SvelteKit routes"
```

---

### Task 6.9: 最终端到端验证

**Step 1: 完整构建**

Run: `bun run build`

**Step 2: Tauri 开发模式测试**

Run: `bun run tauri dev`

验证完整流程：
1. 标题栏拖拽 + 最小化/最大化/关闭到托盘
2. 侧边栏文件夹切换
3. 添加账号弹窗 → 检测服务商 → 填写密码
4. 邮件列表加载 → 点击邮件 → 详情展示
5. 写邮件 → TipTap 富文本 → 发送
6. 搜索邮件
7. 同步按钮 → 状态栏进度
8. 标签显示
9. 日历视图翻月
10. 工作流编辑器节点点击
11. AI 操作区域
12. 设置页面主题/语言切换
13. 系统托盘右键菜单

**Step 3: 修复发现的问题**

**Step 4: 最终 Commit**

```bash
git add -A
git commit -m "feat: Phase 6 complete - tray, error handling, lint, performance, integration"
```

---

### Task 6.10: 清理计划文件 + 版本号更新

**Step 1: 更新 tauri.conf.json 版本**

将版本从 `0.1.0` 更新为 `0.2.0`（如有必要）。

**Step 2: 更新 package.json 版本**

确保 `package.json` 和 `src-tauri/Cargo.toml` 版本一致。

**Step 3: 更新计划状态**

更新 `docs/plans/2026-03-31-refactor-plan.md`，将所有 Phase 标记为"已完成"。

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: update version and mark all phases complete"
```
