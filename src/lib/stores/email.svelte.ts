import { getContext, setContext } from "svelte";
import { commands } from "$lib/bindings";
import type { EmailDto, EmailDetail, EmailCategory } from "$lib/bindings";
import { formatError } from "$lib/utils/error.js";

class EmailState {
  emails = $state<EmailDto[]>([]);
  selectedEmailId = $state<number | null>(null);
  selectedEmail = $state<EmailDetail | null>(null);
  total = $state(0);
  page = $state(1);
  limit = $state(50);
  loading = $state(false);
  currentFolder = $state<EmailCategory>("inbox");

  async loadEmails(accountId: number, folder: string, page = 1) {
    this.loading = true;
    this.currentFolder = "inbox";
    try {
      const result = await commands.listEmails(
        accountId,
        folder,
        page,
        this.limit,
      );
      if (result.status === "ok") {
        this.emails = result.data.emails;
        this.total = result.data.total;
        this.page = result.data.page;
      }
    } catch (e: unknown) {
      console.error("Failed to load emails:", e);
    } finally {
      this.loading = false;
    }
  }

  async loadEmailsByCategory(
    accountId: number,
    category: EmailCategory,
    page = 1,
  ) {
    this.loading = true;
    this.currentFolder = category;
    try {
      const result = await commands.listEmailsByCategory(
        accountId,
        category,
        page,
        this.limit,
      );
      if (result.status === "ok") {
        this.emails = result.data.emails;
        this.total = result.data.total;
        this.page = result.data.page;
      }
    } catch (e: unknown) {
      console.error("Failed to load emails:", e);
    } finally {
      this.loading = false;
    }
  }

  async selectEmail(id: number) {
    this.selectedEmailId = id;
    try {
      const result = await commands.getEmail(id);
      if (result.status === "ok") {
        this.selectedEmail = result.data;
        // 自动标记已读
        if (!this.selectedEmail.is_read) {
          await commands.markAsRead(id, true);
          const email = this.emails.find((e) => e.id === id);
          if (email) email.is_read = true;
        }
      }
    } catch (e: unknown) {
      console.error("Failed to load email:", e);
    }
  }

  deselectEmail() {
    this.selectedEmailId = null;
    this.selectedEmail = null;
  }

  async toggleStar(emailId: number) {
    try {
      const result = await commands.toggleStar(emailId);
      if (result.status === "ok") {
        const newState = result.data;
        const email = this.emails.find((e) => e.id === emailId);
        if (email) email.is_starred = newState;
        if (this.selectedEmail?.id === emailId) {
          this.selectedEmail.is_starred = newState;
        }
      }
    } catch (e: unknown) {
      console.error("Failed to toggle star:", e);
    }
  }

  async deleteEmails(ids: number[]) {
    try {
      const result = await commands.deleteEmails(ids);
      if (result.status === "ok") {
        this.emails = this.emails.filter((e) => !ids.includes(e.id));
        if (this.selectedEmailId && ids.includes(this.selectedEmailId)) {
          this.deselectEmail();
        }
        this.total -= ids.length;
      }
    } catch (e: unknown) {
      console.error("Failed to delete emails:", e);
    }
  }
}

const EMAIL_KEY = Symbol("email");

export function createEmailState() {
  const state = new EmailState();
  setContext(EMAIL_KEY, state);
  return state;
}

export function getEmailState() {
  return getContext<EmailState>(EMAIL_KEY);
}
