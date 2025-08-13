import { faker } from "@faker-js/faker";
import {
  Email,
  EmailAccount,
  Contact,
  Attachment,
  EmailFolder,
  FolderInfo,
  EmailThread,
} from "../types/email";

// 生成随机联系人
export const generateContact = (): Contact => ({
  email: faker.internet.email(),
  name: faker.person.fullName(),
  avatar: faker.image.avatar(),
});

// 生成随机附件
export const generateAttachment = (): Attachment => {
  const fileTypes = [
    { ext: "pdf", mime: "application/pdf" },
    {
      ext: "docx",
      mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    },
    {
      ext: "xlsx",
      mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    },
    { ext: "jpg", mime: "image/jpeg" },
    { ext: "png", mime: "image/png" },
    { ext: "zip", mime: "application/zip" },
  ];

  const fileType = faker.helpers.arrayElement(fileTypes);

  return {
    id: faker.string.uuid(),
    filename: `${faker.system.fileName().split(".")[0]}.${fileType.ext}`,
    size: faker.number.int({ min: 1024, max: 10485760 }), // 1KB to 10MB
    mimeType: fileType.mime,
    url: faker.internet.url(),
  };
};

// 生成邮件账户
export const generateEmailAccount = (): EmailAccount => {
  const providers = ["gmail", "outlook", "yahoo"] as const;
  const provider = faker.helpers.arrayElement(providers);
  const email = faker.internet.email();

  return {
    id: faker.string.uuid(),
    email,
    name: faker.person.fullName(),
    provider,
    isDefault: faker.datatype.boolean(),
    isActive: true,
    color: faker.color.rgb(),
    avatar: faker.image.avatar(),
  };
};

// 生成单个邮件
export const generateEmail = (
  accountId: string,
  folder: EmailFolder = "inbox",
): Email => {
  const hasAttachments = faker.datatype.boolean({ probability: 0.3 });
  const isRead =
    folder === "sent" ? true : faker.datatype.boolean({ probability: 0.6 });
  const bodyHtml = faker.lorem.paragraphs(
    faker.number.int({ min: 1, max: 5 }),
    "<br/><br/>",
  );
  const bodyText = bodyHtml.replace(/<br\/>/g, "\n");

  return {
    id: faker.string.uuid(),
    messageId: `<${faker.string.uuid()}@${faker.internet.domainName()}>`,
    accountId,
    folder,
    from: generateContact(),
    to: Array.from(
      { length: faker.number.int({ min: 1, max: 3 }) },
      generateContact,
    ),
    cc: faker.datatype.boolean({ probability: 0.2 })
      ? Array.from(
          { length: faker.number.int({ min: 1, max: 2 }) },
          generateContact,
        )
      : undefined,
    subject: faker.lorem.sentence({ min: 3, max: 10 }),
    body: bodyText,
    bodyHtml,
    preview: faker.lorem.sentence({ min: 10, max: 20 }),
    date: faker.date.recent({ days: 30 }),
    isRead,
    isStarred: faker.datatype.boolean({ probability: 0.1 }),
    isDraft: folder === "drafts",
    isImportant: faker.datatype.boolean({ probability: 0.15 }),
    isDeleted: folder === "trash",
    hasAttachments,
    attachments: hasAttachments
      ? Array.from(
          { length: faker.number.int({ min: 1, max: 3 }) },
          generateAttachment,
        )
      : [],
    labels: [],
    threadId: faker.string.uuid(),
    isFlagged: faker.datatype.boolean({ probability: 0.05 }),
  };
};

// 生成邮件列表
export const generateEmailList = (
  count: number = 50,
  accountId: string,
  folder: EmailFolder = "inbox",
): Email[] => {
  return Array.from({ length: count }, () =>
    generateEmail(accountId, folder),
  ).sort((a, b) => new Date(b.date).getTime() - new Date(a.date).getTime());
};

// 生成文件夹信息
export const generateFolderInfo = (): FolderInfo[] => {
  const folders: FolderInfo[] = [
    {
      id: "inbox",
      name: "Inbox",
      type: "inbox",
      icon: "inbox",
      count: faker.number.int({ min: 50, max: 200 }),
      unreadCount: faker.number.int({ min: 5, max: 30 }),
      accountId: "",
    },
    {
      id: "sent",
      name: "Sent",
      type: "sent",
      icon: "send",
      count: faker.number.int({ min: 30, max: 100 }),
      unreadCount: 0,
      accountId: "",
    },
    {
      id: "drafts",
      name: "Drafts",
      type: "drafts",
      icon: "drafts",
      count: faker.number.int({ min: 5, max: 20 }),
      unreadCount: 0,
      accountId: "",
    },
    {
      id: "starred",
      name: "Starred",
      type: "starred",
      icon: "star",
      count: faker.number.int({ min: 10, max: 50 }),
      unreadCount: faker.number.int({ min: 0, max: 5 }),
      accountId: "",
    },
    {
      id: "important",
      name: "Important",
      type: "important",
      icon: "label_important",
      count: faker.number.int({ min: 15, max: 60 }),
      unreadCount: faker.number.int({ min: 0, max: 10 }),
      accountId: "",
    },
    {
      id: "spam",
      name: "Spam",
      type: "spam",
      icon: "report",
      count: faker.number.int({ min: 5, max: 30 }),
      unreadCount: faker.number.int({ min: 0, max: 15 }),
      accountId: "",
    },
    {
      id: "trash",
      name: "Trash",
      type: "trash",
      icon: "delete",
      count: faker.number.int({ min: 10, max: 50 }),
      unreadCount: 0,
      accountId: "",
    },
    {
      id: "all",
      name: "All Mail",
      type: "all",
      icon: "mail",
      count: faker.number.int({ min: 200, max: 500 }),
      unreadCount: faker.number.int({ min: 5, max: 30 }),
      accountId: "",
    },
  ];

  return folders;
};

// 生成邮件线程
export const generateEmailThread = (accountId: string): EmailThread => {
  const emailCount = faker.number.int({ min: 2, max: 8 });
  const emails = Array.from({ length: emailCount }, () =>
    generateEmail(accountId, "inbox"),
  ).sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());

  // 设置线程关系
  const threadId = faker.string.uuid();
  emails.forEach((email, index) => {
    email.threadId = threadId;
    if (index > 0) {
      email.inReplyTo = emails[index - 1].messageId;
      email.references = emails.slice(0, index).map((e) => e.messageId);
    }
  });

  const participants = new Map<string, Contact>();
  emails.forEach((email) => {
    participants.set(email.from.email, email.from);
    email.to.forEach((contact) => participants.set(contact.email, contact));
    email.cc?.forEach((contact) => participants.set(contact.email, contact));
  });

  return {
    id: threadId,
    emails,
    subject: emails[0].subject,
    participants: Array.from(participants.values()),
    lastMessageDate: emails[emails.length - 1].date,
    firstMessageDate: emails[0].date,
    unreadCount: emails.filter((e) => !e.isRead).length,
    totalCount: emails.length,
    hasAttachments: emails.some((e) => e.hasAttachments),
    isImportant: emails.some((e) => e.isImportant),
    isStarred: emails.some((e) => e.isStarred),
    labels: [],
    preview:
      emails[emails.length - 1].preview ||
      emails[emails.length - 1].body.substring(0, 100),
  };
};

// Mock 数据存储
class MockEmailStore {
  private accounts: EmailAccount[] = [];
  private emails: Map<string, Email[]> = new Map();

  constructor() {
    this.initializeMockData();
  }

  private initializeMockData() {
    // 创建 2-3 个账户
    const accountCount = faker.number.int({ min: 2, max: 3 });
    this.accounts = Array.from({ length: accountCount }, () => {
      const account = generateEmailAccount();
      return {
        ...account,
        isActive: true,
        unreadCount: 0,
        totalCount: 0,
      };
    });

    // 将第一个账户设为默认
    if (this.accounts.length > 0) {
      this.accounts[0].isDefault = true;
    }

    // 为每个账户生成邮件
    this.accounts.forEach((account) => {
      const emails: Email[] = [];

      // 生成各个文件夹的邮件
      emails.push(...generateEmailList(30, account.id, "inbox"));
      emails.push(...generateEmailList(20, account.id, "sent"));
      emails.push(...generateEmailList(5, account.id, "drafts"));
      emails.push(...generateEmailList(10, account.id, "trash"));
      emails.push(...generateEmailList(5, account.id, "spam"));

      this.emails.set(account.id, emails);
    });
  }

  getAccounts(): EmailAccount[] {
    return this.accounts;
  }

  getEmailsByAccount(accountId: string): Email[] {
    return this.emails.get(accountId) || [];
  }

  getEmailsByFolder(accountId: string, folder: EmailFolder): Email[] {
    const emails = this.emails.get(accountId) || [];

    if (folder === "all") {
      return emails;
    }

    if (folder === "starred") {
      return emails.filter((e) => e.isStarred);
    }

    if (folder === "important") {
      return emails.filter((e) => e.isImportant);
    }

    return emails.filter((e) => e.folder === folder);
  }

  getFolderInfo(accountId: string): FolderInfo[] {
    const folders = generateFolderInfo().map((folder) => ({
      ...folder,
      accountId,
    }));

    // 更新实际计数
    folders.forEach((folder) => {
      const folderEmails = this.getEmailsByFolder(accountId, folder.id);
      folder.count = folderEmails.length;
      folder.unreadCount = folderEmails.filter((e) => !e.isRead).length;
    });

    return folders;
  }
}

export const mockEmailStore = new MockEmailStore();
