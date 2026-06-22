import { describe, it, expect, vi, beforeEach } from "vitest";
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
    });

    expect(mockInvoke).toHaveBeenCalledWith("list_emails_by_category", {
      accountId: 1,
      category: "inbox",
      page: 1,
      limit: 50,
    });
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

  it("处理错误响应", async () => {
    mockInvoke.mockResolvedValue({
      status: "error",
      error: { type: "EmailNotFound", message: "邮件不存在: 999" },
    });
    const result = await invokeMock<EmailDetail>("get_email", { id: 999 });
    expect(result.status).toBe("error");
  });
});
