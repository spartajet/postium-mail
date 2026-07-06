import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  EmailState,
  replaceCidReferences,
} from "$lib/stores/email.svelte";
import { mockInvoke } from "../mocks/tauri";

const email = {
  id: 1,
  account_id: 1,
  folder: "INBOX",
  uid: 100,
  subject: "测试邮件",
  sender_name: "Alice",
  sender_email: "alice@example.com",
  preview: "预览",
  is_read: false,
  is_starred: false,
  sent_at: 1_745_126_400,
  has_attachments: false,
};

const emailDetail = {
  ...email,
  recipient_emails: "me@example.com",
  cc_emails: null,
  body_text: "正文",
  body_html: "<p>正文</p>",
  attachments: [],
};

describe("EmailState 状态行为", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(null);
  });

  it("loadEmailsByCategory 成功后写入列表和分页状态", async () => {
    mockInvoke.mockResolvedValue({ emails: [email], total: 30, page: 2, limit: 50 });
    const state = new EmailState();

    await state.loadEmailsByCategory(1, "starred", 2);

    expect(state.loading).toBe(false);
    expect(state.currentFolder).toBe("starred");
    expect(state.emails).toEqual([email]);
    expect(state.total).toBe(30);
    expect(state.page).toBe(2);
    expect(mockInvoke).toHaveBeenCalledWith("list_emails_by_category", {
      accountId: 1,
      category: "starred",
      page: 2,
      limit: 50,
      unreadOnly: false,
    });
  });

  it("loadEmailsByCategoryForAllAccounts 成功后写入列表和分页状态", async () => {
    mockInvoke.mockResolvedValue({ emails: [email], total: 12, page: 3, limit: 50 });
    const state = new EmailState();
    state.unreadOnly = true;

    await state.loadEmailsByCategoryForAllAccounts("inbox", 3);

    expect(state.loading).toBe(false);
    expect(state.currentFolder).toBe("inbox");
    expect(state.emails).toEqual([email]);
    expect(state.total).toBe(12);
    expect(state.page).toBe(3);
    expect(mockInvoke).toHaveBeenCalledWith(
      "list_emails_by_category_for_all_accounts",
      {
        category: "inbox",
        page: 3,
        limit: 50,
        unreadOnly: true,
      },
    );
  });

  it("loadEmails 按文件夹加载并重置当前分类为 inbox", async () => {
    mockInvoke.mockResolvedValue({ emails: [email], total: 1, page: 1, limit: 50 });
    const state = new EmailState();
    state.currentFolder = "trash";

    await state.loadEmails(1, "Archive", 1);

    expect(state.currentFolder).toBe("inbox");
    expect(state.emails).toHaveLength(1);
    expect(mockInvoke).toHaveBeenCalledWith("list_emails", {
      accountId: 1,
      folder: "Archive",
      page: 1,
      limit: 50,
    });
  });

  it("loadNextPageForAllAccounts 追加下一页结果", async () => {
    mockInvoke.mockResolvedValue({
      emails: [{ ...email, id: 2, subject: "第二页" }],
      total: 2,
      page: 2,
      limit: 50,
    });
    const state = new EmailState();
    state.currentFolder = "starred";
    state.emails = [{ ...email, id: 1, subject: "第一页" }];
    state.total = 2;
    state.page = 1;
    state.unreadOnly = true;

    await state.loadNextPageForAllAccounts();

    expect(state.loadingNextPage).toBe(false);
    expect(state.page).toBe(2);
    expect(state.total).toBe(2);
    expect(state.emails.map((item) => item.id)).toEqual([1, 2]);
    expect(mockInvoke).toHaveBeenCalledWith(
      "list_emails_by_category_for_all_accounts",
      {
        category: "starred",
        page: 2,
        limit: 50,
        unreadOnly: true,
      },
    );
  });

  it("refreshLoadedEmailsByCategoryForAllAccounts 按已加载数量刷新列表", async () => {
    mockInvoke.mockResolvedValue({
      emails: [
        { ...email, id: 1, subject: "刷新后-1" },
        { ...email, id: 2, subject: "刷新后-2" },
      ],
      total: 10,
      page: 1,
      limit: 50,
    });
    const state = new EmailState();
    state.currentFolder = "archive";
    state.emails = [
      { ...email, id: 1, subject: "旧-1" },
      { ...email, id: 2, subject: "旧-2" },
    ];
    state.total = 10;
    state.page = 2;

    await state.refreshLoadedEmailsByCategoryForAllAccounts();

    expect(state.emails.map((item) => item.subject)).toEqual(["刷新后-1", "刷新后-2"]);
    expect(state.total).toBe(10);
    expect(state.page).toBe(1);
    expect(mockInvoke).toHaveBeenCalledWith(
      "list_emails_by_category_for_all_accounts",
      {
        category: "archive",
        page: 1,
        limit: 50,
        unreadOnly: false,
      },
    );
  });

  it("setUnreadOnlyForAllAccounts 切换未读筛选后重新加载当前分类", async () => {
    mockInvoke.mockResolvedValue({ emails: [email], total: 1, page: 1, limit: 50 });
    const state = new EmailState();
    state.currentFolder = "spam";
    state.page = 2;

    await state.setUnreadOnlyForAllAccounts(true);

    expect(state.unreadOnly).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith(
      "list_emails_by_category_for_all_accounts",
      {
        category: "spam",
        page: 1,
        limit: 50,
        unreadOnly: true,
      },
    );
  });

  it("selectEmail 选择未读邮件后加载详情并标记本地列表为已读", async () => {
    mockInvoke
      .mockResolvedValueOnce(emailDetail)
      .mockResolvedValueOnce(null);
    const state = new EmailState();
    state.emails = [{ ...email }];

    await state.selectEmail(1);

    expect(state.selectedEmailId).toBe(1);
    expect(state.selectedEmail?.subject).toBe("测试邮件");
    expect(state.emails[0]!.is_read).toBe(true);
    expect(mockInvoke).toHaveBeenNthCalledWith(1, "get_email", { id: 1 });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "mark_as_read", {
      emailId: 1,
      isRead: true,
    });
  });

  it("toggleStar 同步更新列表和选中详情", async () => {
    mockInvoke.mockResolvedValue(true);
    const state = new EmailState();
    state.emails = [{ ...email }];
    state.selectedEmail = { ...emailDetail };

    await state.toggleStar(1);

    expect(state.emails[0]!.is_starred).toBe(true);
    expect(state.selectedEmail?.is_starred).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith("toggle_star", { emailId: 1 });
  });

  it("markAsRead 成功后同步更新列表和选中详情", async () => {
    mockInvoke.mockResolvedValue(null);
    const state = new EmailState();
    state.emails = [{ ...email }];
    state.selectedEmail = { ...emailDetail };

    const ok = await state.markAsRead(1, true);

    expect(ok).toBe(true);
    expect(state.emails[0]!.is_read).toBe(true);
    expect(state.selectedEmail?.is_read).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith("mark_as_read", {
      emailId: 1,
      isRead: true,
    });
  });

  it("markAsRead 后端失败时不更新本地状态", async () => {
    mockInvoke.mockRejectedValue(new Error("remote failed"));
    const state = new EmailState();
    state.emails = [{ ...email, is_read: false }];
    state.selectedEmail = { ...emailDetail, is_read: false };

    const ok = await state.markAsRead(1, true);

    expect(ok).toBe(false);
    expect(state.emails[0]!.is_read).toBe(false);
    expect(state.selectedEmail?.is_read).toBe(false);
    expect(state.error).toContain("remote failed");
  });

  it("reloadEmail 收到 reloaded 时更新列表和当前详情", async () => {
    const state = new EmailState();
    state.emails = [
      {
        ...email,
        id: 1,
        subject: "旧主题",
        preview: "旧预览",
        account_email: "account@example.com",
        account_display_name: "账号来源",
      },
    ];
    state.selectedEmailId = 1;
    state.selectedEmail = { ...emailDetail, subject: "旧主题", body_text: "旧正文" };
    const reloadedEmail = { ...email, id: 1, subject: "新主题", preview: "新预览" };
    mockInvoke.mockResolvedValueOnce({
      status: "reloaded",
      email: { ...emailDetail, ...reloadedEmail, body_text: "新正文" },
    });

    const ok = await state.reloadEmail(1);

    expect(ok).toBe(true);
    expect(state.emails[0]?.subject).toBe("新主题");
    expect(state.emails[0]?.account_email).toBe("account@example.com");
    expect(state.emails[0]?.account_display_name).toBe("账号来源");
    expect(state.selectedEmail?.body_text).toBe("新正文");
    expect(mockInvoke).toHaveBeenCalledWith("reload_email", { emailId: 1 });
  });

  it("reloadEmail 收到 removed 时移除列表并取消选择", async () => {
    const state = new EmailState();
    state.emails = [{ ...email, id: 1 }, { ...email, id: 2 }];
    state.total = 2;
    state.selectedEmailId = 1;
    state.selectedEmail = { ...emailDetail, id: 1 };
    mockInvoke.mockResolvedValueOnce({
      status: "removed",
      email_id: 1,
    });

    const ok = await state.reloadEmail(1);

    expect(ok).toBe(true);
    expect(state.emails.map((email) => email.id)).toEqual([2]);
    expect(state.selectedEmailId).toBeNull();
    expect(state.selectedEmail).toBeNull();
    expect(state.total).toBe(1);
  });

  it("reloadEmail 后端失败时保留本地状态", async () => {
    const state = new EmailState();
    state.emails = [{ ...email, id: 1, subject: "旧主题" }];
    mockInvoke.mockRejectedValueOnce(new Error("remote failed"));

    const ok = await state.reloadEmail(1);

    expect(ok).toBe(false);
    expect(state.emails[0]?.subject).toBe("旧主题");
    expect(state.error).toContain("remote failed");
  });

  it("archiveEmail 成功后从当前列表移除并取消选择", async () => {
    mockInvoke.mockResolvedValue(null);
    const state = new EmailState();
    state.emails = [{ ...email }];
    state.total = 1;
    state.selectedEmailId = 1;
    state.selectedEmail = { ...emailDetail };

    const ok = await state.archiveEmail(1);

    expect(ok).toBe(true);
    expect(state.emails).toEqual([]);
    expect(state.total).toBe(0);
    expect(state.selectedEmailId).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("archive_email", { emailId: 1 });
  });

  it("archiveEmail 后端失败时保留当前列表", async () => {
    mockInvoke.mockRejectedValue(new Error("archive failed"));
    const state = new EmailState();
    state.emails = [{ ...email }];
    state.total = 1;

    const ok = await state.archiveEmail(1);

    expect(ok).toBe(false);
    expect(state.emails).toHaveLength(1);
    expect(state.total).toBe(1);
    expect(state.error).toContain("archive failed");
  });

  it("moveEmailToFolder 成功后从当前列表移除", async () => {
    mockInvoke.mockResolvedValue(null);
    const state = new EmailState();
    state.emails = [{ ...email }];
    state.total = 1;

    const ok = await state.moveEmailToFolder(1, "Work");

    expect(ok).toBe(true);
    expect(state.emails).toEqual([]);
    expect(state.total).toBe(0);
    expect(mockInvoke).toHaveBeenCalledWith("move_email_to_folder", {
      emailId: 1,
      folder: "Work",
    });
  });

  it("moveEmailToFolder 后端失败时保留当前列表", async () => {
    mockInvoke.mockRejectedValue(new Error("move failed"));
    const state = new EmailState();
    state.emails = [{ ...email }];
    state.total = 1;

    const ok = await state.moveEmailToFolder(1, "Work");

    expect(ok).toBe(false);
    expect(state.emails).toHaveLength(1);
    expect(state.total).toBe(1);
    expect(state.error).toContain("move failed");
  });

  it("deleteEmails 移除邮件、取消当前选择并扣减总数", async () => {
    mockInvoke.mockResolvedValue(1);
    const state = new EmailState();
    state.emails = [
      { ...email, id: 1 },
      { ...email, id: 2, subject: "保留邮件" },
    ];
    state.total = 2;
    state.selectedEmailId = 1;
    state.selectedEmail = { ...emailDetail };

    await state.deleteEmails([1]);

    expect(state.emails.map((item) => item.id)).toEqual([2]);
    expect(state.selectedEmailId).toBeNull();
    expect(state.selectedEmail).toBeNull();
    expect(state.total).toBe(1);
    expect(mockInvoke).toHaveBeenCalledWith("delete_emails", { emailIds: [1] });
  });

  it("downloadAttachment 下载后更新当前详情中的附件缓存状态", async () => {
    const attachment = {
      id: 7,
      email_id: 1,
      filename: "a.txt",
      content_type: "text/plain",
      size: 12,
      disposition: "attachment",
      content_id: null,
      is_inline: false,
      is_cached: false,
      cache_path: null,
    };
    const state = new EmailState();
    state.selectedEmail = {
      ...emailDetail,
      has_attachments: true,
      attachments: [attachment],
    };
    mockInvoke.mockResolvedValueOnce({
      ...attachment,
      is_cached: true,
      cache_path: "/tmp/a.txt",
    });

    await state.downloadAttachment(7);

    expect(state.selectedEmail.attachments[0]?.is_cached).toBe(true);
    expect(state.selectedEmail.attachments[0]?.cache_path).toBe("/tmp/a.txt");
    expect(state.attachmentOperatingIds.has(7)).toBe(false);
    expect(mockInvoke).toHaveBeenCalledWith("ensure_attachment_cached", {
      attachmentId: 7,
    });
  });

  it("saveAttachmentAs 保存失败时记录当前附件错误", async () => {
    const state = new EmailState();
    mockInvoke.mockRejectedValueOnce(new Error("save failed"));

    await state.saveAttachmentAs(7, "/tmp/a.txt");

    expect(state.attachmentErrors[7]).toContain("save failed");
    expect(state.attachmentOperatingIds.has(7)).toBe(false);
  });

  it("replaces cid references case-insensitively and url-encoded", () => {
    const html = '<p><img src="cid:Logo@Example.Com"><img src="cid:logo%40example.com"></p>';
    const result = replaceCidReferences(html, [
      {
        content_id: "<logo@example.com>",
        url: "asset://localhost/logo.png",
      },
    ]);

    expect(result).toContain('src="asset://localhost/logo.png"');
    expect(result).not.toContain("cid:Logo@Example.Com");
    expect(result).not.toContain("cid:logo%40example.com");
  });

  it("忽略旧邮件延迟返回的 CID 解析结果", async () => {
    const state = new EmailState();
    const first = {
      ...emailDetail,
      id: 1,
      body_html: '<img src="cid:first@example.com">',
      attachments: [
        {
          id: 8,
          email_id: 1,
          filename: "first.png",
          content_type: "image/png",
          size: 100,
          disposition: "inline",
          content_id: "<first@example.com>",
          is_inline: true,
          is_cached: false,
          cache_path: null,
        },
      ],
    };
    const second = {
      ...emailDetail,
      id: 2,
      body_html: "<p>second</p>",
      attachments: [],
    };
    mockInvoke.mockResolvedValueOnce([
      { content_id: "<first@example.com>", url: "/tmp/first.png" },
    ]);
    state.selectedEmail = first;
    const resolvingFirst = state.resolveInlineAttachmentsForSelectedEmail();
    state.selectedEmail = second;
    state.resolvedBodyHtml = second.body_html;

    await resolvingFirst;

    expect(state.resolvedBodyHtml).toBe("<p>second</p>");
  });
});
