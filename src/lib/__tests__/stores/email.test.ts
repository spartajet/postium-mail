import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  commands,
} from "$lib/bindings";
import { EmailState } from "$lib/stores/email.svelte";
import {
  createMockResult,
  expectOk,
  type MockCommandResult,
  mockInvoke,
} from "../mocks/tauri";

// Import after mock
const { invoke } = await import("@tauri-apps/api/core");

type Email = {
  id: number;
  account_id: number;
  folder: string;
  uid: number;
  subject: string;
  sender_name: string;
  sender_email: string;
  preview: string;
  is_read: boolean;
  is_starred: boolean;
  sent_at: number;
  has_attachments: boolean;
};

type EmailList = {
  emails: Email[];
  total: number;
  page: number;
  limit: number;
};

type EmailDetail = Email & {
  recipient_emails: string;
  cc_emails: string | null;
  body_text: string;
  body_html: string;
};

function invokeMock<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<MockCommandResult<T>> {
  return invoke<MockCommandResult<T>>(command, args);
}

describe("EmailState invoke 测试", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.restoreAllMocks();
  });

  it("listEmails 调用正确的命令和参数", async () => {
    const mockData = {
      emails: [
        {
          id: 1,
          account_id: 1,
          folder: "INBOX",
          uid: 100,
          subject: "Test Email",
          sender_name: "Alice",
          sender_email: "alice@test.com",
          preview: "Hello...",
          is_read: false,
          is_starred: false,
          sent_at: 1745126400,
          has_attachments: false,
        },
      ],
      total: 1,
      page: 1,
      limit: 50,
    };
    mockInvoke.mockResolvedValue(createMockResult(mockData));

    const result = await invokeMock<EmailList>("list_emails", {
      accountId: 1,
      folder: "INBOX",
      page: 1,
      limit: 50,
    });

    expect(mockInvoke).toHaveBeenCalledWith("list_emails", {
      accountId: 1,
      folder: "INBOX",
      page: 1,
      limit: 50,
    });
    expect(result.status).toBe("ok");
    expectOk(result);
    expect(result.data.emails).toHaveLength(1);
  });

  it("listEmailsByCategory 调用正确", async () => {
    mockInvoke.mockResolvedValue(
      createMockResult({ emails: [], total: 0, page: 1, limit: 50 }),
    );

    await invokeMock<EmailList>("list_emails_by_category", {
      accountId: 1,
      category: "inbox",
      page: 1,
      limit: 50,
      unreadOnly: false,
    });

    expect(mockInvoke).toHaveBeenCalledWith("list_emails_by_category", {
      accountId: 1,
      category: "inbox",
      page: 1,
      limit: 50,
      unreadOnly: false,
    });
  });

  it("loadNextPage 追加下一页分类邮件并更新分页状态", async () => {
    const firstEmail = {
      id: 1,
      account_id: 1,
      folder: "INBOX",
      uid: 100,
      subject: "First",
      sender_name: "Alice",
      sender_email: "alice@test.com",
      preview: "Hello",
      is_read: false,
      is_starred: false,
      sent_at: 1745126400,
      has_attachments: false,
    };
    const secondEmail = {
      ...firstEmail,
      id: 2,
      uid: 101,
      subject: "Second",
    };
    const state = new EmailState();
    state.emails = [firstEmail];
    state.total = 2;
    state.page = 1;
    state.currentFolder = "inbox";

    const listEmailsByCategorySpy = vi
      .spyOn(commands, "listEmailsByCategory")
      .mockResolvedValue(
        createMockResult({
          emails: [secondEmail],
          total: 2,
          page: 2,
          limit: 50,
        }),
      );

    await state.loadNextPage(1);

    expect(listEmailsByCategorySpy).toHaveBeenCalledWith(
      1,
      "inbox",
      2,
      50,
      false,
    );
    expect(state.emails).toEqual([firstEmail, secondEmail]);
    expect(state.total).toBe(2);
    expect(state.page).toBe(2);
  });

  it("未读模式按分类从后端分页加载所有未读邮件", async () => {
    const firstEmail = {
      id: 1,
      account_id: 1,
      folder: "INBOX",
      uid: 100,
      subject: "Unread",
      sender_name: "Alice",
      sender_email: "alice@test.com",
      preview: "Hello",
      is_read: false,
      is_starred: false,
      sent_at: 1745126400,
      has_attachments: false,
    };
    const secondEmail = {
      ...firstEmail,
      id: 2,
      uid: 101,
      subject: "Older unread",
    };
    const state = new EmailState();

    const listEmailsByCategorySpy = vi
      .spyOn(commands, "listEmailsByCategory")
      .mockResolvedValueOnce(
        createMockResult({
          emails: [firstEmail],
          total: 2,
          page: 1,
          limit: 50,
        }),
      )
      .mockResolvedValueOnce(
        createMockResult({
          emails: [secondEmail],
          total: 2,
          page: 2,
          limit: 50,
        }),
      );

    await state.setUnreadOnly(1, true);
    await state.loadNextPage(1);

    expect(listEmailsByCategorySpy).toHaveBeenNthCalledWith(
      1,
      1,
      "inbox",
      1,
      50,
      true,
    );
    expect(listEmailsByCategorySpy).toHaveBeenNthCalledWith(
      2,
      1,
      "inbox",
      2,
      50,
      true,
    );
    expect(state.unreadOnly).toBe(true);
    expect(state.emails).toEqual([firstEmail, secondEmail]);
    expect(state.total).toBe(2);
  });

  it("loadNextPage 不使用整列表 loading，避免追加分页时列表被骨架屏替换", async () => {
    const firstEmail = {
      id: 1,
      account_id: 1,
      folder: "INBOX",
      uid: 100,
      subject: "First",
      sender_name: "Alice",
      sender_email: "alice@test.com",
      preview: "Hello",
      is_read: false,
      is_starred: false,
      sent_at: 1745126400,
      has_attachments: false,
    };
    const state = new EmailState();
    state.emails = [firstEmail];
    state.total = 2;
    state.page = 1;
    state.currentFolder = "inbox";

    let resolveRequest!: (
      value: Awaited<ReturnType<typeof commands.listEmailsByCategory>>,
    ) => void;
    const pendingRequest = new Promise<
      Awaited<ReturnType<typeof commands.listEmailsByCategory>>
    >((resolve) => {
      resolveRequest = resolve;
    });
    vi.spyOn(commands, "listEmailsByCategory").mockReturnValue(pendingRequest);

    const loadPromise = state.loadNextPage(1);

    expect(state.loading).toBe(false);
    expect(state.loadingNextPage).toBe(true);

    resolveRequest(
      createMockResult({
        emails: [],
        total: 1,
        page: 2,
        limit: 50,
      }),
    );
    await loadPromise;

    expect(state.loadingNextPage).toBe(false);
  });

  it("refreshLoadedEmailsByCategory 刷新当前已加载范围并保留页数", async () => {
    const firstEmail = {
      id: 1,
      account_id: 1,
      folder: "INBOX",
      uid: 100,
      subject: "First",
      sender_name: "Alice",
      sender_email: "alice@test.com",
      preview: "Hello",
      is_read: false,
      is_starred: false,
      sent_at: 1745126400,
      has_attachments: false,
    };
    const secondEmail = {
      ...firstEmail,
      id: 2,
      uid: 101,
      subject: "Older",
    };
    const state = new EmailState();
    state.emails = [firstEmail];
    state.total = 1;
    state.page = 1;
    state.currentFolder = "inbox";

    const listEmailsByCategorySpy = vi
      .spyOn(commands, "listEmailsByCategory")
      .mockResolvedValue(
        createMockResult({
          emails: [firstEmail, secondEmail],
          total: 2,
          page: 1,
        limit: 50,
      }),
    );

    await state.refreshLoadedEmailsByCategory(1, 1);

    expect(listEmailsByCategorySpy).toHaveBeenCalledWith(
      1,
      "inbox",
      1,
      50,
      false,
    );
    expect(state.emails).toEqual([firstEmail, secondEmail]);
    expect(state.total).toBe(2);
    expect(state.page).toBe(1);
    expect(state.loading).toBe(false);
  });

  it("getEmail 并在未读时调用 markAsRead", async () => {
    const emailDetail = {
      id: 1,
      account_id: 1,
      folder: "INBOX",
      uid: 100,
      subject: "Detail",
      sender_name: "Bob",
      sender_email: "bob@test.com",
      preview: "preview",
      is_read: false,
      is_starred: false,
      sent_at: 1745126400,
      has_attachments: false,
      recipient_emails: "me@test.com",
      cc_emails: null,
      body_text: "Hello",
      body_html: "<p>Hello</p>",
    };

    mockInvoke
      .mockResolvedValueOnce(createMockResult(emailDetail))
      .mockResolvedValueOnce(createMockResult(null));

    const result = await invokeMock<EmailDetail>("get_email", { id: 1 });
    expectOk(result);
    expect(result.data.is_read).toBe(false);

    // 标记已读使用 emailId 参数名（与 bindings 中的签名一致）
    await invokeMock<null>("mark_as_read", { emailId: 1, isRead: true });
    expect(mockInvoke).toHaveBeenCalledWith("mark_as_read", {
      emailId: 1,
      isRead: true,
    });
  });

  it("toggleStar 调用正确", async () => {
    mockInvoke.mockResolvedValue(createMockResult(true));
    const result = await invokeMock<boolean>("toggle_star", { emailId: 1 });
    expectOk(result);
    expect(result.data).toBe(true);
  });

  it("deleteEmails 调用正确", async () => {
    mockInvoke.mockResolvedValue(createMockResult(2));
    const result = await invokeMock<number>("delete_emails", { emailIds: [1, 2] });
    expectOk(result);
    expect(result.data).toBe(2);
  });

  it("archiveEmail 调用正确", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await commands.archiveEmail(123);

    expect(result.status).toBe("ok");
    expect(mockInvoke).toHaveBeenCalledWith("archive_email", { emailId: 123 });
  });

  it("处理错误响应", async () => {
    mockInvoke.mockResolvedValue({
      status: "error",
      error: { type: "EmailNotFound", message: "邮件不存在: 999" },
    });
    const result = await invokeMock<EmailDetail>("get_email", { id: 999 });
    expect(result.status).toBe("error");
  });
});
