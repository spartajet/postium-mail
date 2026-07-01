/**
 * Postium Mail - 邮件状态管理模块
 * email.svelte.ts
 *
 * 本模块提供邮件的状态管理功能，使用 Svelte 5 的 Runes API 实现响应式状态。
 *
 * ==================== 功能概述 ====================
 * 1. 管理当前文件夹的邮件列表
 * 2. 跟踪选中的邮件及其详情
 * 3. 支持邮件的分页加载
 * 4. 提供邮件的星标切换、删除等操作
 * 5. 使用 Svelte Context API 实现跨组件状态共享
 *
 * ==================== 设计原则 ====================
 * 1. 单例模式：通过 Svelte Context 确保全局只有一个 EmailState 实例
 * 2. 响应式：使用 $state 和 $derived Runes 实现自动更新
 * 3. 错误处理：统一处理 API 调用错误，提供用户友好的错误消息
 * 4. 类型安全：完整的 TypeScript 类型定义
 * 5. 性能优化：支持分页加载，避免一次性加载过多数据
 *
 * ==================== 使用示例 ====================
 * ```typescript
 * // 在 +layout.svelte 中初始化
 * import { createEmailState } from '$lib/stores/email.svelte';
 * const emailState = createEmailState();
 *
 * // 在子组件中获取状态
 * import { getEmailState } from '$lib/stores/email.svelte';
 * const emailState = getEmailState();
 *
 * // 加载收件箱邮件
 * await emailState.loadEmailsByCategory(accountId, 'inbox', 1);
 *
 * // 选择邮件查看详情
 * await emailState.selectEmail(emailId);
 *
 * // 切换星标
 * await emailState.toggleStar(emailId);
 *
 * // 删除邮件
 * await emailState.deleteEmails([emailId]);
 * ```
 */

// 导入 Svelte Context API，用于跨组件状态共享
import { getContext, setContext } from "svelte";

// 导入 Tauri 命令绑定，用于调用后端 API
import { convertFileSrc } from "@tauri-apps/api/core";
import { commands } from "$lib/bindings";

// 导入邮件相关的数据传输对象类型
import type {
  AttachmentDto,
  EmailDto,
  EmailDetail,
  EmailCategory,
  InlineAttachmentDto,
} from "$lib/bindings";

// 导入错误格式化工具
import { formatError } from "$lib/utils/error.js";

/**
 * 邮件状态管理类
 *
 * 管理当前视图中的邮件列表、选中邮件的详情、分页状态等信息。
 * 使用 Svelte 5 的 Runes API ($state) 实现响应式状态管理。
 *
 * ==================== 状态字段 ====================
 * - `emails`: 当前文件夹的邮件列表（响应式）
 * - `selectedEmailId`: 当前选中的邮件 ID（响应式）
 * - `selectedEmail`: 当前选中邮件的完整详情（响应式）
 * - `total`: 邮件总数（用于分页计算）
 * - `page`: 当前页码
 * - `limit`: 每页显示数量
 * - `loading`: 是否正在加载（响应式）
 * - `currentFolder`: 当前查看的邮件分类（响应式）
 *
 * ==================== 生命周期 ====================
 * 1. 应用启动时，由 +layout.svelte 创建 EmailState 实例
 * 2. 通过 setContext 将实例存入 Svelte Context
 * 3. 子组件通过 getContext 获取同一个实例
 * 4. 所有状态变更自动触发 UI 更新
 *
 * ==================== 注意事项 ====================
 * - 此类不应直接实例化，应通过 createEmailState() 创建
 * - 状态字段使用 $state 标记，支持响应式更新
 * - selectedEmail 在选择邮件时自动加载完整详情
 * - 选择邮件时会自动标记为已读
 */
export class EmailState {
  /** 当前文件夹的邮件列表（响应式状态） */
  emails = $state<EmailDto[]>([]);

  /** 当前选中的邮件 ID（响应式状态），null 表示没有选中 */
  selectedEmailId = $state<number | null>(null);

  /** 当前选中邮件的完整详情（响应式状态），null 表示没有选中 */
  selectedEmail = $state<EmailDetail | null>(null);

  /** 邮件总数（用于分页计算和显示） */
  total = $state(0);

  /** 当前页码（从 1 开始） */
  page = $state(1);

  /** 每页显示的邮件数量 */
  limit = $state(50);

  /** 是否正在加载邮件数据（响应式状态） */
  loading = $state(false);

  /** 是否正在追加下一页邮件，不替换当前列表 */
  loadingNextPage = $state(false);

  /** 最近一次邮件操作错误，空字符串表示无错误 */
  error = $state("");

  /** 正在执行远端操作的邮件 ID 集合 */
  operatingIds = $state<Set<number>>(new Set());

  /** 正在执行附件操作的附件 ID 集合 */
  attachmentOperatingIds = $state<Set<number>>(new Set());

  /** 按附件 ID 保存的操作错误 */
  attachmentErrors = $state<Record<number, string>>({});

  /** 已替换 CID 图片后的正文 HTML */
  resolvedBodyHtml = $state<string | null>(null);

  /** 当前查看的邮件分类（响应式状态），如 "inbox"、"sent" 等 */
  currentFolder = $state<EmailCategory>("inbox");

  /** 是否只显示未读邮件 */
  unreadOnly = $state(false);

  private inlineResolveRequestId = 0;

  private beginOperation(emailId: number) {
    this.error = "";
    this.operatingIds = new Set([...this.operatingIds, emailId]);
  }

  private endOperation(emailId: number) {
    const next = new Set(this.operatingIds);
    next.delete(emailId);
    this.operatingIds = next;
  }

  private setError(e: unknown, fallback: string) {
    this.error = e instanceof Error ? e.message : fallback;
    console.error(fallback, e);
  }

  private beginAttachmentOperation(attachmentId: number) {
    this.attachmentErrors = { ...this.attachmentErrors, [attachmentId]: "" };
    this.attachmentOperatingIds = new Set([
      ...this.attachmentOperatingIds,
      attachmentId,
    ]);
  }

  private endAttachmentOperation(attachmentId: number) {
    const next = new Set(this.attachmentOperatingIds);
    next.delete(attachmentId);
    this.attachmentOperatingIds = next;
  }

  private replaceSelectedAttachment(updated: AttachmentDto) {
    if (!this.selectedEmail) return;

    this.selectedEmail = {
      ...this.selectedEmail,
      attachments: this.selectedEmail.attachments.map((attachment) =>
        attachment.id === updated.id ? updated : attachment,
      ),
    };
  }

  private setAttachmentError(
    attachmentId: number,
    error: unknown,
    fallback: string,
  ) {
    const message = formatError(error);
    this.attachmentErrors = {
      ...this.attachmentErrors,
      [attachmentId]: message || fallback,
    };
  }

  private removeFromCurrentList(emailId: number) {
    const before = this.emails.length;
    this.emails = this.emails.filter((email) => email.id !== emailId);
    if (this.total > 0 && this.emails.length < before) {
      this.total -= 1;
    }
    if (this.selectedEmailId === emailId) {
      this.deselectEmail();
    }
  }

  /**
   * 按文件夹加载邮件列表
   *
   * 从指定文件夹（如 "INBOX"、"Sent" 等）加载邮件列表。
   * 支持分页加载，避免一次性获取过多数据。
   *
   * ==================== 工作流程 ====================
   * 1. 设置 loading 状态为 true
   * 2. 调用后端 listEmails 命令获取邮件列表
   * 3. 成功时更新 emails、total、page 状态
   * 4. 失败时记录错误日志
   * 5. 最终将 loading 设置为 false
   *
   * ==================== 注意事项 ====================
   * - 此方法会将 currentFolder 强制设置为 "inbox"
   * - 如果需要按分类加载，请使用 loadEmailsByCategory() 方法
   * - 分页从 1 开始（不是 0）
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const emailState = getEmailState();
   * await emailState.loadEmails(accountId, 'INBOX', 1);
   * console.log(`加载了 ${emailState.emails.length} 封邮件`);
   * ```
   *
   * @param accountId - 账号 ID
   * @param folder - 文件夹名称（如 "INBOX"、"Sent"、"Drafts"）
   * @param page - 页码（默认为 1）
   * @returns Promise<void>
   */
  async loadEmails(accountId: number, folder: string, page = 1) {
    // 设置加载状态
    this.loading = true;
    // 强制设置当前分类为 inbox
    this.currentFolder = "inbox";

    try {
      // 调用后端命令获取邮件列表
      const result = await commands.listEmails(
        accountId,
        folder,
        page,
        this.limit,
      );

      if (result.status === "ok") {
        // 成功时更新状态
        this.emails = result.data.emails;
        this.total = result.data.total;
        this.page = result.data.page;
      }
    } catch (e: unknown) {
      // 记录错误日志
      console.error("Failed to load emails:", e);
    } finally {
      // 无论成功或失败，都重置加载状态
      this.loading = false;
    }
  }

  /**
   * 按分类加载邮件列表
   *
   * 根据邮件分类（如收件箱、已发送、星标等）加载邮件列表。
   * 这是推荐的邮件加载方式，因为它使用统一的分类枚举，
   * 不需要关心底层 IMAP 文件夹的具体名称。
   *
   * ==================== 支持的分类 ====================
   * - "inbox": 收件箱
   * - "starred": 星标邮件（跨文件夹）
   * - "sent": 已发送
   * - "drafts": 草稿箱
   * - "spam": 垃圾邮件
   * - "trash": 已删除
   * - "archive": 归档
   *
   * ==================== 工作流程 ====================
   * 1. 设置 loading 状态为 true
   * 2. 更新 currentFolder 为请求的分类
   * 3. 调用后端 listEmailsByCategory 命令
   * 4. 成功时更新 emails、total、page 状态
   * 5. 失败时记录错误日志
   * 6. 最终将 loading 设置为 false
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const emailState = getEmailState();
   *
   * // 加载收件箱
   * await emailState.loadEmailsByCategory(accountId, 'inbox');
   *
   * // 加载星标邮件
   * await emailState.loadEmailsByCategory(accountId, 'starred');
   *
   * // 加载第二页
   * await emailState.loadEmailsByCategory(accountId, 'inbox', 2);
   * ```
   *
   * @param accountId - 账号 ID
   * @param category - 邮件分类（EmailCategory 类型）
   * @param page - 页码（默认为 1）
   * @returns Promise<void>
   */
  async loadEmailsByCategory(
    accountId: number,
    category: EmailCategory,
    page = 1,
  ) {
    // 设置加载状态
    this.loading = true;
    // 更新当前分类
    this.currentFolder = category;

    try {
      // 调用后端命令按分类获取邮件列表
      const result = await commands.listEmailsByCategory(
        accountId,
        category,
        page,
        this.limit,
        this.unreadOnly,
      );

      if (result.status === "ok") {
        // 成功时更新状态
        this.emails = result.data.emails;
        this.total = result.data.total;
        this.page = result.data.page;
      }
    } catch (e: unknown) {
      // 记录错误日志
      console.error("Failed to load emails:", e);
    } finally {
      // 无论成功或失败，都重置加载状态
      this.loading = false;
    }
  }

  async loadNextPage(accountId: number) {
    if (this.loading || this.loadingNextPage || this.emails.length >= this.total)
      return;

    const nextPage = this.page + 1;
    this.loadingNextPage = true;

    try {
      const result = await commands.listEmailsByCategory(
        accountId,
        this.currentFolder,
        nextPage,
        this.limit,
        this.unreadOnly,
      );

      if (result.status === "ok") {
        this.emails = [...this.emails, ...result.data.emails];
        this.total = result.data.total;
        this.page = result.data.page;
      }
    } catch (e: unknown) {
      this.setError(e, "Failed to load next email page");
    } finally {
      this.loadingNextPage = false;
    }
  }

  async refreshLoadedEmailsByCategory(accountId: number, minimumLimit = 0) {
    const loadedCount = Math.max(this.emails.length, minimumLimit, this.limit);

    try {
      const result = await commands.listEmailsByCategory(
        accountId,
        this.currentFolder,
        1,
        loadedCount,
        this.unreadOnly,
      );

      if (result.status === "ok") {
        this.emails = result.data.emails;
        this.total = result.data.total;
        this.page = Math.max(1, Math.ceil(result.data.emails.length / this.limit));
      }
    } catch (e: unknown) {
      this.setError(e, "Failed to refresh loaded emails");
    }
  }

  async setUnreadOnly(accountId: number, unreadOnly: boolean) {
    if (this.unreadOnly === unreadOnly && this.page === 1) return;
    this.unreadOnly = unreadOnly;
    await this.loadEmailsByCategory(accountId, this.currentFolder, 1);
  }

  /**
   * 选择邮件并加载详情
   *
   * 选中指定的邮件，并从后端获取完整的邮件详情（包括正文、附件等）。
   * 如果邮件未读，会自动将其标记为已读。
   *
   * ==================== 工作流程 ====================
   * 1. 更新 selectedEmailId 为选中的邮件 ID
   * 2. 调用后端 getEmail 命令获取完整邮件详情
   * 3. 成功时更新 selectedEmail 状态
   * 4. 如果邮件未读，调用 markAsRead 标记为已读
   * 5. 更新本地邮件列表中的已读状态
   * 6. 失败时记录错误日志
   *
   * ==================== 注意事项 ====================
   * - 选择邮件后会自动加载完整详情（包括大段的正文和附件信息）
   * - 自动标记已读功能可以防止用户手动标记
   * - 邮件详情包含完整的 MIME 内容，可能较大
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const emailState = getEmailState();
   *
   * // 选择邮件
   * await emailState.selectEmail(123);
   *
   * // 访问邮件详情
   * if (emailState.selectedEmail) {
   *   console.log('主题:', emailState.selectedEmail.subject);
   *   console.log('正文:', emailState.selectedEmail.body_html);
   * }
   * ```
   *
   * @param id - 要选择的邮件 ID
   * @returns Promise<void>
   */
  async selectEmail(id: number) {
    // 更新选中的邮件 ID
    this.selectedEmailId = id;

    try {
      // 调用后端命令获取邮件详情
      const result = await commands.getEmail(id);

      if (result.status === "ok") {
        // 更新选中邮件的详情
        this.selectedEmail = result.data;
        this.resolvedBodyHtml = result.data.body_html;
        void this.resolveInlineAttachmentsForSelectedEmail();

        // 自动标记已读
        // 如果邮件当前是未读状态，调用后端标记为已读
        if (!this.selectedEmail.is_read) {
          await this.markAsRead(id, true);
        }
      }
    } catch (e: unknown) {
      // 记录错误日志
      console.error("Failed to load email:", e);
    }
  }

  /**
   * 取消选择邮件
   *
   * 清除当前选中的邮件，将 selectedEmailId 和 selectedEmail 都设为 null。
   * 通常在用户返回邮件列表或切换文件夹时调用。
   *
   * ==================== 使用场景 ====================
   * - 用户点击"返回"按钮回到邮件列表
   * - 用户切换到其他文件夹
   * - 删除当前选中的邮件后
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const emailState = getEmailState();
   *
   * // 取消选择
   * emailState.deselectEmail();
   *
   * // 检查是否有选中的邮件
   * if (!emailState.selectedEmailId) {
   *   console.log('没有选中任何邮件');
   * }
   * ```
   */
  deselectEmail() {
    this.selectedEmailId = null;
    this.selectedEmail = null;
    this.resolvedBodyHtml = null;
    this.attachmentErrors = {};
    this.attachmentOperatingIds = new Set();
    this.inlineResolveRequestId += 1;
  }

  async markAsRead(emailId: number, isRead: boolean): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.markAsRead(emailId, isRead);
      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }

      const email = this.emails.find((item) => item.id === emailId);
      if (email) email.is_read = isRead;

      if (this.selectedEmail?.id === emailId) {
        this.selectedEmail.is_read = isRead;
      }
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to mark email read state");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }

  async reloadEmail(emailId: number): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.reloadEmail(emailId);

      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }

      if (result.data.status === "reloaded") {
        const detail = result.data.email;
        const updatedEmail: EmailDto = {
          id: detail.id,
          account_id: detail.account_id,
          folder: detail.folder,
          uid: detail.uid,
          subject: detail.subject,
          sender_name: detail.sender_name,
          sender_email: detail.sender_email,
          preview: detail.preview,
          is_read: detail.is_read,
          is_starred: detail.is_starred,
          sent_at: detail.sent_at,
          has_attachments: detail.has_attachments,
        };
        this.emails = this.emails.map((email) =>
          email.id === emailId ? updatedEmail : email,
        );
        if (this.selectedEmailId === emailId) {
          this.selectedEmail = detail;
          this.resolvedBodyHtml = detail.body_html;
          void this.resolveInlineAttachmentsForSelectedEmail();
        }
        return true;
      }

      const removedId = result.data.email_id;
      const before = this.emails.length;
      this.emails = this.emails.filter((email) => email.id !== removedId);
      const removed = before - this.emails.length;
      if (this.selectedEmailId === removedId) {
        this.deselectEmail();
      }
      this.total = Math.max(0, this.total - removed);
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to reload email");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }

  async archiveEmail(emailId: number): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.archiveEmail(emailId);
      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }
      this.removeFromCurrentList(emailId);
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to archive email");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }

  async moveEmailToFolder(emailId: number, folder: string): Promise<boolean> {
    this.beginOperation(emailId);
    try {
      const result = await commands.moveEmailToFolder(emailId, folder);
      if (result.status === "error") {
        this.error = formatError(result.error);
        return false;
      }
      this.removeFromCurrentList(emailId);
      return true;
    } catch (e: unknown) {
      this.setError(e, "Failed to move email");
      return false;
    } finally {
      this.endOperation(emailId);
    }
  }

  /**
   * 切换邮件星标状态
   *
   * 切换指定邮件的星标（收藏）状态。
   * 如果当前是星标，则取消；如果当前不是星标，则添加。
   *
   * ==================== 工作流程 ====================
   * 1. 调用后端 toggleStar 命令切换状态
   * 2. 成功时更新本地邮件列表中的星标状态
   * 3. 如果该邮件是当前选中的邮件，同时更新 selectedEmail 的状态
   * 4. 失败时记录错误日志
   *
   * ==================== 注意事项 ====================
   * - 这是一个切换操作，不需要传递目标状态
   * - 后端会返回切换后的新状态
   * - 更新会同时反映在列表和详情视图中
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const emailState = getEmailState();
   *
   * // 切换星标
   * await emailState.toggleStar(123);
   *
   * // 检查结果
   * const email = emailState.emails.find(e => e.id === 123);
   * if (email?.is_starred) {
   *   console.log('已加星标');
   * }
   * ```
   *
   * @param emailId - 要切换星标的邮件 ID
   * @returns Promise<void>
   */
  async toggleStar(emailId: number) {
    this.beginOperation(emailId);
    try {
      // 调用后端命令切换星标状态
      const result = await commands.toggleStar(emailId);

      if (result.status === "error") {
        this.error = formatError(result.error);
        return;
      }

      // 获取切换后的新状态
      const newState = result.data;

      // 更新本地邮件列表中的星标状态
      const email = this.emails.find((e) => e.id === emailId);
      if (email) email.is_starred = newState;

      // 如果该邮件是当前选中的邮件，同时更新详情视图的状态
      if (this.selectedEmail?.id === emailId) {
        this.selectedEmail.is_starred = newState;
      }
    } catch (e: unknown) {
      this.setError(e, "Failed to toggle star");
    } finally {
      this.endOperation(emailId);
    }
  }

  /**
   * 删除邮件
   *
   * 删除指定的邮件。第一阶段仅支持单封邮件。
   * 第一阶段删除语义是移动到服务商配置的 Trash 文件夹，
   * 不执行永久删除或 EXPUNGE。
   *
   * ==================== 工作流程 ====================
   * 1. 调用后端 deleteEmails 命令删除邮件
   * 2. 成功时从本地邮件列表中移除已删除的邮件
   * 3. 如果删除的是当前选中的邮件，取消选择
   * 4. 更新邮件总数
   * 5. 失败时记录错误日志
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const emailState = getEmailState();
   *
   * // 删除单封邮件
   * await emailState.deleteEmails([123]);
   *
   * ```
   *
   * @param ids - 要删除的邮件 ID 数组
   * @returns Promise<void>
   */
  async deleteEmails(ids: number[]) {
    ids.forEach((id) => this.beginOperation(id));
    try {
      // 调用后端命令删除邮件
      const result = await commands.deleteEmails(ids);

      if (result.status === "error") {
        this.error = formatError(result.error);
        return;
      }

      // 从本地列表中移除已删除的邮件
      const before = this.emails.length;
      this.emails = this.emails.filter((e) => !ids.includes(e.id));
      const removed = before - this.emails.length;

      // 如果删除的是当前选中的邮件，取消选择
      if (this.selectedEmailId && ids.includes(this.selectedEmailId)) {
        this.deselectEmail();
      }

      // 更新邮件总数
      this.total = Math.max(0, this.total - removed);
    } catch (e: unknown) {
      this.setError(e, "Failed to delete emails");
    } finally {
      ids.forEach((id) => this.endOperation(id));
    }
  }

  async refreshCurrentCategory(accountId: number) {
    try {
      await this.loadEmailsByCategory(accountId, this.currentFolder, this.page);
    } catch (e: unknown) {
      this.setError(e, "Failed to refresh emails");
    }
  }

  async downloadAttachment(attachmentId: number) {
    this.beginAttachmentOperation(attachmentId);
    try {
      const result = await commands.ensureAttachmentCached(attachmentId);
      if (result.status === "ok") {
        this.replaceSelectedAttachment(result.data);
      } else {
        this.setAttachmentError(attachmentId, result.error, "附件下载失败");
      }
    } catch (error: unknown) {
      this.setAttachmentError(attachmentId, error, "附件下载失败");
    } finally {
      this.endAttachmentOperation(attachmentId);
    }
  }

  async saveAttachmentAs(attachmentId: number, targetPath: string) {
    this.beginAttachmentOperation(attachmentId);
    try {
      const result = await commands.saveAttachmentAs(attachmentId, targetPath);
      if (result.status === "error") {
        this.setAttachmentError(attachmentId, result.error, "附件保存失败");
      }
    } catch (error: unknown) {
      this.setAttachmentError(attachmentId, error, "附件保存失败");
    } finally {
      this.endAttachmentOperation(attachmentId);
    }
  }

  async openAttachment(attachmentId: number) {
    this.beginAttachmentOperation(attachmentId);
    try {
      const result = await commands.openAttachment(attachmentId);
      if (result.status === "error") {
        this.setAttachmentError(attachmentId, result.error, "附件打开失败");
      }
    } catch (error: unknown) {
      this.setAttachmentError(attachmentId, error, "附件打开失败");
    } finally {
      this.endAttachmentOperation(attachmentId);
    }
  }

  async resolveInlineAttachmentsForSelectedEmail() {
    const email = this.selectedEmail;
    const requestId = ++this.inlineResolveRequestId;
    if (!email?.body_html) {
      this.resolvedBodyHtml = email?.body_html ?? null;
      return;
    }

    const smallInlineImages = email.attachments.filter(
      (attachment) =>
        attachment.content_id &&
        attachment.content_type.startsWith("image/") &&
        attachment.size <= 10 * 1024 * 1024,
    );
    if (smallInlineImages.length === 0) {
      this.resolvedBodyHtml = email.body_html;
      return;
    }

    const result = await commands.resolveInlineAttachments(email.id);
    if (requestId !== this.inlineResolveRequestId || this.selectedEmail?.id !== email.id) {
      return;
    }
    if (result.status !== "ok") {
      this.resolvedBodyHtml = email.body_html;
      return;
    }

    const mapped = result.data.map((attachment) => ({
      ...attachment,
      url: convertFileSrc(attachment.url),
    }));
    this.resolvedBodyHtml = replaceCidReferences(email.body_html, mapped);
  }
}

export function normalizeContentId(contentId: string): string {
  return contentId.trim().replace(/^<|>$/g, "");
}

export function replaceCidReferences(
  html: string,
  inlineAttachments: InlineAttachmentDto[],
): string {
  let next = html;
  for (const attachment of inlineAttachments) {
    const cid = normalizeContentId(attachment.content_id);
    const candidates = new Set([cid, encodeURIComponent(cid)]);
    for (const candidate of candidates) {
      const escaped = candidate.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      next = next.replace(new RegExp(`cid:${escaped}`, "gi"), attachment.url);
    }
  }
  return next;
}

/**
 * Svelte Context 的键名
 *
 * 使用 Symbol 作为键名，确保在 Svelte Context 中的唯一性，
 * 避免与其他模块的 Context 键名冲突。
 */
const EMAIL_KEY = Symbol("email");

/**
 * 创建邮件状态管理器
 *
 * 创建一个新的 EmailState 实例并将其存入 Svelte Context。
 * 此函数应在应用的根布局组件（+layout.svelte）中调用，
 * 确保所有子组件都能访问同一个状态实例。
 *
 * ==================== 使用说明 ====================
 * - 只在应用初始化时调用一次
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例可以通过 getEmailState() 在子组件中获取
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- +layout.svelte -->
 * <script lang="ts">
 *   import { createEmailState } from '$lib/stores/email.svelte';
 *
 *   // 创建并设置 Context
 *   const emailState = createEmailState();
 * </script>
 * ```
 *
 * @returns EmailState 实例
 */
export function createEmailState() {
  const state = new EmailState();
  // 将状态实例存入 Svelte Context，供子组件获取
  setContext(EMAIL_KEY, state);
  return state;
}

/**
 * 获取邮件状态管理器
 *
 * 从 Svelte Context 中获取由 createEmailState() 创建的 EmailState 实例。
 * 此函数应在子组件中调用，以访问全局的邮件状态。
 *
 * ==================== 使用说明 ====================
 * - 调用此函数前，必须确保父组件已调用 createEmailState()
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例与 createEmailState() 创建的是同一个对象
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- EmailList.svelte -->
 * <script lang="ts">
 *   import { getEmailState } from '$lib/stores/email.svelte';
 *
 *   // 获取全局邮件状态
 *   const emailState = getEmailState();
 * </script>
 *
 * {#each emailState.emails as email}
 *   <div class="email-item" class:selected={email.id === emailState.selectedEmailId}>
 *     {email.subject}
 *   </div>
 * {/each}
 * ```
 *
 * @returns EmailState 实例
 * @throws 如果在 createEmailState() 之前调用，会抛出 Context 错误
 */
export function getEmailState() {
  return getContext<EmailState>(EMAIL_KEY);
}
