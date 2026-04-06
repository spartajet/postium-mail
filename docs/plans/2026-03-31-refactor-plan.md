# Postium Mail 重构实施计划（索引）

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将旧 Vue 3 + Naive UI 邮件客户端完全重构为 Svelte 5 + shadcn-svelte + Tailwind CSS v4 + 四层 Rust 后端架构。

**Architecture:** 后端 Command → Service → Domain → Infrastructure 四层分离，tauri-specta 自动生成 TypeScript 绑定。前端 SvelteKit SPA 模式，三栏邮件布局 + Glassmorphism 风格。

**Tech Stack:** Tauri 2 / Rust / SeaORM 2.0.0-rc / tauri-specta 2.0.0-rc.20 / Svelte 5 / SvelteKit 2 / shadcn-svelte (next) / Tailwind CSS v4 / TipTap / TypeScript 5

**Design Doc:** `docs/plans/2026-03-31-refactor-design.md`

---

## Phase 文件索引

| Phase | 文件 | 内容 | 状态 |
|-------|------|------|------|
| Phase 0 | [phase-0-project-infra.md](phase-0-project-infra.md) | 项目基建：依赖、Tailwind、模块骨架、Migration、前端目录 | 已完成 |
| Phase 1 | [phase-1-backend-foundation.md](phase-1-backend-foundation.md) | 后端基础：MailError、Provider traits、个人服务商、IMAP/SMTP、AuthManager | 已完成 |
| Phase 2 | [phase-2-backend-service-command.md](phase-2-backend-service-command.md) | 后端 Service+Command：Repository、FTS5、Service 层、Command 层、specta Builder | 已完成 |
| Phase 3 | [phase-3-frontend-infra.md](phase-3-frontend-infra.md) | 前端基建：TitleBar、WindowControls、Sidebar、AppShell、StatusBar、Store、i18n | 已完成 |
| Phase 4 | [phase-4-frontend-core.md](phase-4-frontend-core.md) | 前端核心：AddAccount、EmailList、EmailDetail、ComposeModal、SyncProgress、邮件操作 | 已完成 |
| Phase 5 | [phase-5-frontend-advanced.md](phase-5-frontend-advanced.md) | 前端高级：标签 UI、日历 UI、工作流 UI、AI 操作 UI（后端 stub） | 已完成 |
| Phase 6 | [phase-6-integration-cleanup.md](phase-6-integration-cleanup.md) | 集成清理：系统托盘、错误处理、Lint/Clippy、性能优化、E2E 验证 | 已完成 |

## 执行顺序

严格按 Phase 0 → 6 顺序执行。每个 Phase 内的 Task 按编号顺序执行。

## 依赖关系图

```
Phase 0 (基建)
  ├── Phase 1 (后端 Domain) ──→ Phase 2 (后端 Service/Command)
  └── Phase 3 (前端基建)    ──→ Phase 4 (前端核心) ──→ Phase 5 (前端高级)
                                   ↑________________________|
Phase 2 + Phase 4 ──→ Phase 6 (集成)
```
