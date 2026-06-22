import { beforeEach, describe, expect, it, vi } from "vitest";
import { EmailState } from "$lib/stores/email.svelte";
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
};

describe("EmailState 状态行为", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
    });
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
});
