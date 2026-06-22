/**
 * Postium Mail - SvelteKit 路由布局配置文件
 * +layout.ts
 *
 * 本文件配置整个应用的路由渲染行为，特别是与 Tauri 桌面应用框架的集成。
 *
 * ==================== 配置说明 ====================
 *
 * Tauri 应用架构特点：
 * - Tauri 是一个构建跨平台桌面应用的框架，使用系统 WebView 渲染前端
 * - Tauri 应用运行在本地，没有传统的 Node.js 服务器环境
 * - 所有静态资源、JavaScript 代码都在用户本地执行
 *
 * 因此，传统的服务端渲染（Server-Side Rendering, SSR）在 Tauri 中：
 * 1. 不需要：因为没有服务器来执行 Node.js 代码和生成 HTML
 * 2. 不适用：应用直接加载静态文件，不需要服务端预渲染
 *
 * ==================== 配置项详解 ====================
 *
 * 本配置将应用设置为单页应用（SPA）模式：
 * - 所有路由切换都在客户端完成，无需页面刷新
 * - 使用 adapter-static 适配器生成静态文件
 * - 配置 fallback 到 index.html，确保路由正确处理
 *
 * 相关文档：
 * - SvelteKit 单页应用模式：https://svelte.dev/docs/kit/single-page-apps
 * - Tauri + SvelteKit 集成：https://v2.tauri.app/start/frontend/sveltekit/
 */

/**
 * 禁用服务端渲染（Server-Side Rendering）
 *
 * 作用：
 * - 告诉 SvelteKit 不要在服务器端预渲染页面 HTML
 * - 所有页面都在客户端浏览器中动态渲染
 * - 适用于完全运行在客户端的应用（如 SPA、桌面应用）
 *
 * 为什么设为 false：
 * 1. Tauri 应用没有服务器端环境，无法执行 SSR
 * 2. 应用作为静态文件运行，由 WebView 加载
 * 3. 所有数据通过 Tauri invoke API 与 Rust 后端通信，不需要服务器中间层
 *
 * 影响：
 * - 页面加载时可能看到短暂的加载状态（可优化）
 * - SEO 不是问题（因为是桌面应用，不需要搜索引擎索引）
 * - 更好的用户体验（无页面刷新，流畅的客户端导航）
 */
export const ssr = false;

/**
 * 禁用预渲染（Prerendering）
 *
 * 作用：
 * - 告诉 SvelteKit 在构建时不要预生成静态 HTML 文件
 * - 所有页面都是动态的，在客户端运行时渲染
 *
 * 为什么设为 false：
 * 1. 应用内容是动态的（邮件数据、账户信息等），无法预渲染
 * 2. 邮件内容来自 Tauri 后端数据库，构建时不存在
 * 3. 用户数据需要登录后才能访问，预渲染没有意义
 *
 * 影响：
 * - 构建速度更快（不需要预渲染每个路由）
 * - 部署更简单（只需生成一套客户端代码）
 * - 首屏加载时间可能稍长（需要等待客户端 JS 执行）
 *
 * 注意：
 * - 配合 adapter-static 使用时，会生成一个 index.html 作为入口
 * - 所有路由都会回退到 index.html，由前端路由器处理
 * - 这是典型的 SPA 部署模式
 */
export const prerender = false;
