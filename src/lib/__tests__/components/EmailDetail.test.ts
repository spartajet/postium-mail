import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import EmailDetail from "$lib/components/email/EmailDetail.svelte";

const downloadAttachment = vi.fn();
const saveAttachmentAs = vi.fn();
const openAttachment = vi.fn();

const selectedEmail = {
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

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            email: {
                noEmailSelected: "选择一封邮件开始阅读",
                from: "发件人",
                to: "收件人",
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
    it("渲染真实附件并触发下载", async () => {
        render(EmailDetail);

        expect(screen.getByText("report.pdf")).toBeTruthy();
        await fireEvent.click(screen.getByRole("button", { name: "下载" }));

        expect(downloadAttachment).toHaveBeenCalledWith(7);
    });
});
