import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import EmailDetail from "$lib/components/email/EmailDetail.svelte";

const downloadAttachment = vi.fn();
const saveAttachmentAs = vi.fn();
const openAttachment = vi.fn();

const baseSelectedEmail = {
    id: 1,
    account_id: 1,
    folder: "INBOX",
    uid: 10,
    subject: "subject",
    sender_name: "Alice",
    sender_email: "alice@example.com",
    preview: null,
    is_read: true,
    is_starred: false,
    sent_at: 1,
    has_attachments: true,
    recipient_emails: "bob@example.com",
    cc_emails: null,
    body_text: null,
    body_html: "<p>Hello</p>",
    attachments: [
        {
            id: 7,
            email_id: 1,
            filename: "report.pdf",
            content_type: "application/pdf",
            size: 1024,
            disposition: "attachment",
            content_id: null,
            is_inline: false,
            is_cached: false,
            cache_path: null,
        },
    ],
};

let selectedEmail = { ...baseSelectedEmail };

const accounts = [
    {
        id: 1,
        name: "Main",
        email: "gxz04220427@163.com",
        display_name: null,
        provider: "imap",
        color: null,
        sync_enabled: true,
        auth_type: "password",
        account_type: "personal",
        last_sync_at: null,
        created_at: 1,
    },
];

vi.mock("@tauri-apps/plugin-dialog", () => ({
    save: vi.fn(),
}));

vi.mock("$lib/stores/email.svelte", () => ({
    getEmailState: () => ({
        selectedEmail,
        resolvedBodyHtml: "<p>Hello</p>",
        attachmentOperatingIds: new Set(),
        attachmentErrors: {},
        operatingIds: new Set(),
        downloadAttachment,
        saveAttachmentAs,
        openAttachment,
        toggleStar: vi.fn(),
        archiveEmail: vi.fn(),
        deleteEmails: vi.fn(),
    }),
}));

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        accounts,
    }),
}));

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            email: {
                noEmailSelected: "选择一封邮件开始阅读",
                from: "发件人",
                to: "收件人",
                otherRecipients: "及其他 {count} 个收件人",
                expandRecipients: "展开收件人",
                collapseRecipients: "收起收件人",
                cc: "抄送",
                attachments: "附件",
                attachmentDownload: "下载",
                attachmentOpen: "打开",
                attachmentSaveAs: "另存为",
                attachmentSave: "保存",
                attachmentCached: "已缓存",
                attachmentNotDownloaded: "未下载",
                reply: "回复",
                forward: "转发",
                star: "星标",
                archive: "归档",
            },
            ai: {
                summary: "AI 摘要",
                smartReply: "智能回复",
                translate: "翻译",
                tasks: "提取任务",
                provider: "助手",
            },
            common: {
                delete: "删除",
            },
        },
    }),
}));

describe("EmailDetail", () => {
    beforeEach(() => {
        selectedEmail = { ...baseSelectedEmail };
        vi.clearAllMocks();
    });

    it("多收件人默认折叠并可展开完整列表", async () => {
        selectedEmail = {
            ...selectedEmail,
            recipient_emails:
                "alice@example.com, gxz04220427@163.com, bob@example.com",
        };

        render(EmailDetail);

        expect(
            screen.getByText("gxz04220427@163.com 及其他 2 个收件人"),
        ).toBeTruthy();
        expect(screen.queryByText("alice@example.com")).toBeNull();

        await fireEvent.click(
            screen.getByRole("button", { name: "展开收件人" }),
        );

        expect(screen.getByText("alice@example.com")).toBeTruthy();
        expect(screen.getByText("bob@example.com")).toBeTruthy();
    });

    it("渲染真实附件并触发下载", async () => {
        render(EmailDetail);

        expect(screen.getByText("report.pdf")).toBeTruthy();
        await fireEvent.click(screen.getByRole("button", { name: "下载" }));

        expect(downloadAttachment).toHaveBeenCalledWith(7);
    });
});
