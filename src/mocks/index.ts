import { faker } from "@faker-js/faker/locale/zh_CN";
import type {
  Email,
  Account,
  CalendarEvent,
  Attachment,
  EmailFolder,
  EmailProvider,
  RepeatType,
} from "@/types";
import { AccountType, AuthType } from "@/types";

// 配置 faker 种子以保持数据一致性（可选）
// faker.seed(123)

// 预定义颜色列表
const COLORS = [
  "#7C3AED",
  "#3B82F6",
  "#10B981",
  "#F59E0B",
  "#EF4444",
  "#EC4899",
  "#8B5CF6",
  "#06B6D4",
  "#F97316",
  "#84CC16",
  "#14B8A6",
  "#6366F1",
];

/**
 * 格式化文件大小
 */
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  return (bytes / (1024 * 1024)).toFixed(1) + " MB";
}

/**
 * 生成附件
 */
export function generateAttachment(): Attachment {
  const extensions = ["pdf", "doc", "docx", "xls", "xlsx", "jpg", "png", "zip"];
  const ext = faker.helpers.arrayElement(extensions);
  const name = faker.system.commonFileName(ext);
  const size = faker.number.int({ min: 5000, max: 10000000 });

  return {
    name,
    size: formatFileSize(size),
  };
}

/**
 * 生成附件列表
 */
export function generateAttachments(count: number = 0): Attachment[] {
  if (count === 0) {
    // 随机生成 0-3 个附件
    count = faker.number.int({ min: 0, max: 3 });
  }
  return Array.from({ length: count }, () => generateAttachment());
}

/**
 * 生成单封邮件
 */
export function generateEmail(overrides?: Partial<Email>): Email {
  const folder =
    overrides?.folder ||
    faker.helpers.arrayElement<EmailFolder>([
      "inbox",
      "inbox",
      "inbox",
      "sent",
      "drafts",
      "spam",
    ]);
  const isSent = folder === "sent";

  return {
    id: faker.string.uuid(),
    sender: faker.person.fullName(),
    senderEmail: faker.internet.email(),
    recipient: faker.internet.email(),
    subject: faker.lorem.sentence({ min: 3, max: 10 }),
    preview: faker.lorem.paragraph({ min: 1, max: 2 }).slice(0, 100),
    body: generateEmailBody(),
    date: faker.date.recent({ days: 30 }),
    unread: !isSent && faker.datatype.boolean({ probability: 0.4 }),
    starred: faker.datatype.boolean({ probability: 0.15 }),
    labels: generateLabels(),
    folder,
    attachments: generateAttachments(),
    accountId: overrides?.accountId || "acc-1",
    ...overrides,
  };
}

/**
 * 生成邮件正文（HTML 格式）
 */
function generateEmailBody(): string {
  const paragraphs = faker.lorem.paragraphs({ min: 2, max: 5 });
  return paragraphs
    .split("\n")
    .map((p) => `<p>${p}</p>`)
    .join("");
}

/**
 * 生成标签
 */
function generateLabels(): string[] {
  const allLabels = [
    "work",
    "urgent",
    "personal",
    "finance",
    "travel",
    "newsletter",
  ];
  const count = faker.number.int({ min: 0, max: 2 });
  return faker.helpers.arrayElements(allLabels, count);
}

/**
 * 生成邮件列表
 */
export function generateEmails(
  count: number = 50,
  accountId?: string,
): Email[] {
  const emails: Email[] = [];

  // 生成收件箱邮件（约60%）
  const inboxCount = Math.floor(count * 0.6);
  for (let i = 0; i < inboxCount; i++) {
    emails.push(
      generateEmail({
        folder: "inbox",
        accountId: accountId || "acc-1",
        id: `email-${i + 1}`,
      }),
    );
  }

  // 生成已发送邮件（约20%）
  const sentCount = Math.floor(count * 0.2);
  for (let i = 0; i < sentCount; i++) {
    emails.push(
      generateEmail({
        folder: "sent",
        accountId: accountId || "acc-1",
        id: `email-${inboxCount + i + 1}`,
        sender: "我",
        senderEmail: "me@postium.com",
      }),
    );
  }

  // 生成草稿（约10%）
  const draftsCount = Math.floor(count * 0.1);
  for (let i = 0; i < draftsCount; i++) {
    emails.push(
      generateEmail({
        folder: "drafts",
        accountId: accountId || "acc-1",
        id: `email-${inboxCount + sentCount + i + 1}`,
        unread: false,
      }),
    );
  }

  // 生成垃圾邮件（约10%）
  const remaining = count - inboxCount - sentCount - draftsCount;
  for (let i = 0; i < remaining; i++) {
    emails.push(
      generateEmail({
        folder: "spam",
        accountId: accountId || "acc-1",
        id: `email-${inboxCount + sentCount + draftsCount + i + 1}`,
      }),
    );
  }

  // 按日期排序（最新的在前）
  emails.sort((a, b) => b.date.getTime() - a.date.getTime());

  return emails;
}

/**
 * 生成单个账号
 */
export function generateAccount(overrides?: Partial<Account>): Account {
  const providers: EmailProvider[] = [
    "gmail",
    "outlook",
    "icloud",
    "yahoo",
    "imap",
  ];
  const provider = overrides?.provider || faker.helpers.arrayElement(providers);

  const accountNames: Record<EmailProvider, string[]> = {
    gmail: ["工作邮箱", "个人邮箱", "订阅邮箱"],
    outlook: ["学校邮箱", "公司邮箱"],
    icloud: ["iCloud 邮箱", "Apple 邮箱"],
    yahoo: ["Yahoo 邮箱"],
    imap: ["自定义邮箱", "企业邮箱"],
  };

  const name =
    overrides?.name || faker.helpers.arrayElement(accountNames[provider]);

  const now = new Date();

  return {
    id: faker.string.uuid(),
    name,
    email: faker.internet.email(),
    provider,
    accountType: AccountType.Personal,
    authType: AuthType.Password,
    color: faker.helpers.arrayElement(COLORS),
    unreadCount: faker.number.int({ min: 0, max: 50 }),
    createdAt: now,
    updatedAt: now,
    ...overrides,
  };
}

/**
 * 生成账号列表
 */
export function generateAccounts(count: number = 3): Account[] {
  const accounts: Account[] = [];

  // 第一个账号（主账号）
  accounts.push(
    generateAccount({
      id: "acc-1",
      name: "工作邮箱",
      email: "work@postium.com",
      provider: "gmail",
      color: "#7C3AED",
      unreadCount: 12,
    }),
  );

  // 第二个账号
  if (count >= 2) {
    accounts.push(
      generateAccount({
        id: "acc-2",
        name: "个人邮箱",
        email: "personal@outlook.com",
        provider: "outlook",
        color: "#3B82F6",
        unreadCount: 5,
      }),
    );
  }

  // 第三个账号
  if (count >= 3) {
    accounts.push(
      generateAccount({
        id: "acc-3",
        name: "订阅邮箱",
        email: "newsletter@gmail.com",
        provider: "gmail",
        color: "#10B981",
        unreadCount: 28,
      }),
    );
  }

  return accounts;
}

/**
 * 生成日历事件
 */
export function generateCalendarEvent(
  overrides?: Partial<CalendarEvent>,
): CalendarEvent {
  const startTime = faker.number.int({ min: 8, max: 18 });
  const endTime = startTime + faker.number.int({ min: 1, max: 3 });

  return {
    id: faker.string.uuid(),
    title: faker.helpers.arrayElement([
      "项目会议",
      "团队周会",
      "产品评审",
      "客户电话",
      "代码审查",
      "培训课程",
      "面试",
      "午餐",
      "出差",
      "截止日期",
    ]),
    date: faker.date.soon({ days: 30 }),
    startTime: `${startTime.toString().padStart(2, "0")}:00`,
    endTime: `${Math.min(endTime, 20).toString().padStart(2, "0")}:00`,
    repeat: faker.helpers.arrayElement<RepeatType>([
      "none",
      "none",
      "none",
      "daily",
      "weekly",
      "monthly",
    ]),
    color: faker.helpers.arrayElement(COLORS),
    notes: faker.lorem.sentence(),
    ...overrides,
  };
}

/**
 * 生成日历事件列表
 */
export function generateCalendarEvents(count: number = 10): CalendarEvent[] {
  const events: CalendarEvent[] = [];

  for (let i = 0; i < count; i++) {
    // 分布在未来 30 天内
    const daysOffset = faker.number.int({ min: 0, max: 30 });
    const date = new Date();
    date.setDate(date.getDate() + daysOffset);

    events.push(
      generateCalendarEvent({
        id: `event-${i + 1}`,
        date,
      }),
    );
  }

  // 按日期排序
  events.sort((a, b) => a.date.getTime() - b.date.getTime());

  return events;
}

/**
 * Mock AI 摘要生成
 */
export function mockAISummary(email: Email): string {
  const points = [
    `来自 ${email.sender} 的邮件，主题为"${email.subject}"`,
    faker.lorem.sentence({ min: 5, max: 12 }),
    faker.lorem.sentence({ min: 5, max: 12 }),
  ];

  return points.map((p) => `• ${p}`).join("\n");
}

/**
 * Mock 智能回复生成
 */
export function mockSmartReply(
  tone: "formal" | "casual" | "brief" = "brief",
): string[] {
  const replies: Record<string, string[]> = {
    formal: [
      "尊敬的先生/女士：\n\n感谢您的来信，关于您提到的内容，我会尽快处理并给您回复。\n\n此致\n敬礼",
      "您好：\n\n已收到您的邮件，相关事宜正在处理中，预计1-3个工作日内完成。\n\n如有疑问，请随时联系。\n\n谢谢",
      "尊敬的客户：\n\n感谢您对我们的支持。您的反馈我们已经收到，会认真考虑并改进。\n\n祝您生活愉快！",
    ],
    casual: [
      "收到！我看一下，晚点回复你~",
      "好的，没问题！我处理一下哈",
      "嘿嘿，收到啦！这个事我知道了~",
    ],
    brief: [
      "收到，我会尽快处理。",
      "好的，了解了。",
      "已收到，谢谢。",
      "好的，我会跟进的。",
    ],
  };

  return faker.helpers.arrayElements(replies[tone], { min: 2, max: 3 });
}

/**
 * Mock AI 写作助手
 */
export function mockAICompose(
  action: "generate" | "improve" | "shorten" | "formal",
  context?: string,
): string {
  const results: Record<string, string[]> = {
    generate: [
      "您好，\n\n感谢您抽出时间与我沟通。关于我们之前讨论的事项，我想跟进一下进度。\n\n请问您方便的时候能否给我一个简短的更新？如果有任何需要我协助的地方，请随时告知。\n\n期待您的回复。\n\n此致\n敬礼",
    ],
    improve: [
      context ? `经过优化后的内容：\n\n${context}` : "请提供需要改进的内容。",
    ],
    shorten: [
      context
        ? context.split("。").slice(0, 2).join("。") + "。"
        : "请提供需要精简的内容。",
    ],
    formal: [
      context
        ? `尊敬的先生/女士：\n\n${context}\n\n此致\n敬礼`
        : "请提供需要正式化的内容。",
    ],
  };

  return faker.helpers.arrayElement(results[action]);
}

/**
 * 模拟延迟（用于异步操作）
 */
export function delay(ms: number = 500): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * 导出所有 mock 数据生成器
 */
export const mockData = {
  generateEmail,
  generateEmails,
  generateAccount,
  generateAccounts,
  generateCalendarEvent,
  generateCalendarEvents,
  generateAttachment,
  generateAttachments,
  mockAISummary,
  mockSmartReply,
  mockAICompose,
  delay,
};

export default mockData;
