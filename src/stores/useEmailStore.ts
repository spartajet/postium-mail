import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import { v4 as uuidv4 } from "uuid";
import {
  Email,
  EmailAccount,
  EmailFolder,
  FolderInfo,
  EmailFilter,
  EmailSort,
  ComposeDraft,
  EmailThread,
  Label,
  SyncStatus,
} from "../types/email";
import { mockEmailStore } from "../mocks/emailMocks";
import { emailLogger, storeLogger } from "../utils/logger";
import { enableMapSet } from "immer";

enableMapSet();

interface EmailState {
  // 账户相关
  accounts: EmailAccount[];
  activeAccountIds: Set<string>;
  currentAccountId: string | null;

  // 邮件相关
  emails: Map<string, Email[]>; // accountId -> emails
  emailThreads: Map<string, EmailThread[]>; // accountId -> threads
  selectedEmailIds: Set<string>;
  currentEmail: Email | null;
  currentThread: EmailThread | null;

  // 文件夹相关
  folders: Map<string, FolderInfo[]>; // accountId -> folders
  currentFolderId: string;
  expandedFolders: Set<string>;

  // 过滤和排序
  filter: EmailFilter;
  sort: EmailSort;
  searchTerm: string;
  viewMode: "list" | "conversation" | "compact";

  // 标签
  labels: Map<string, Label[]>; // accountId -> labels

  // 撰写邮件
  composeDrafts: ComposeDraft[];
  activeComposeDraftId: string | null;

  // 同步状态
  syncStatus: Map<string, SyncStatus>; // accountId -> syncStatus
  isSyncing: boolean;
  lastSyncTime: Date | null;

  // UI 状态
  isLoading: boolean;
  error: string | null;
  selectedPane: "folders" | "list" | "detail";
  isPaneCollapsed: {
    folders: boolean;
    list: boolean;
    detail: boolean;
  };
  paneWidths: {
    folders: number;
    list: number;
    detail: number;
  };

  // Actions
  // 账户操作
  addAccount: (account: EmailAccount) => Promise<void>;
  removeAccount: (accountId: string) => Promise<void>;
  updateAccount: (accountId: string, updates: Partial<EmailAccount>) => void;
  setCurrentAccount: (accountId: string) => void;
  toggleAccountActive: (accountId: string) => void;
  switchAccount: (accountId: string) => Promise<void>;

  // 邮件操作
  loadEmails: (
    accountId?: string,
    folderId?: string,
    force?: boolean,
  ) => Promise<void>;
  loadEmailsForAllAccounts: () => Promise<void>;
  refreshCurrentFolder: () => Promise<void>;
  selectEmail: (email: Email) => void;
  selectThread: (thread: EmailThread) => void;
  toggleEmailSelection: (emailId: string) => void;
  selectAllEmails: () => void;
  clearSelection: () => void;

  // 邮件动作
  markAsRead: (emailIds: string[]) => Promise<void>;
  markAsUnread: (emailIds: string[]) => Promise<void>;
  toggleStar: (emailId: string) => Promise<void>;
  toggleFlag: (emailId: string) => Promise<void>;
  toggleImportant: (emailId: string) => Promise<void>;
  deleteEmails: (emailIds: string[]) => Promise<void>;
  permanentlyDeleteEmails: (emailIds: string[]) => Promise<void>;
  moveToFolder: (emailIds: string[], folderId: string) => Promise<void>;
  archiveEmails: (emailIds: string[]) => Promise<void>;
  markAsSpam: (emailIds: string[]) => Promise<void>;

  // 文件夹操作
  selectFolder: (folderId: string) => Promise<void>;
  createFolder: (
    accountId: string,
    name: string,
    parentId?: string,
  ) => Promise<void>;
  renameFolder: (folderId: string, newName: string) => Promise<void>;
  deleteFolder: (folderId: string) => Promise<void>;
  toggleFolderExpanded: (folderId: string) => void;
  refreshFolders: (accountId?: string) => Promise<void>;

  // 标签操作
  createLabel: (
    accountId: string,
    name: string,
    color: string,
  ) => Promise<void>;
  updateLabel: (labelId: string, updates: Partial<Label>) => Promise<void>;
  deleteLabel: (labelId: string) => Promise<void>;
  addLabelToEmails: (emailIds: string[], labelId: string) => Promise<void>;
  removeLabelFromEmails: (emailIds: string[], labelId: string) => Promise<void>;

  // 搜索和过滤
  setSearchTerm: (term: string) => void;
  setFilter: (filter: Partial<EmailFilter>) => void;
  setSort: (sort: EmailSort) => void;
  clearFilters: () => void;
  setViewMode: (mode: "list" | "conversation" | "compact") => void;

  // 撰写邮件
  createDraft: (draft?: Partial<ComposeDraft>) => string;
  updateDraft: (draftId: string, updates: Partial<ComposeDraft>) => void;
  deleteDraft: (draftId: string) => void;
  sendEmail: (draftId: string) => Promise<void>;
  saveDraft: (draftId: string) => Promise<void>;
  setActiveDraft: (draftId: string | null) => void;

  // 同步操作
  syncAccount: (accountId: string) => Promise<void>;
  syncFolder: (accountId: string, folderId: string) => Promise<void>;
  syncAllAccounts: () => Promise<void>;
  stopSync: () => void;

  // UI 操作
  setPaneWidth: (pane: "folders" | "list" | "detail", width: number) => void;
  togglePaneCollapse: (pane: "folders" | "list" | "detail") => void;
  setSelectedPane: (pane: "folders" | "list" | "detail") => void;

  // 初始化
  initialize: () => Promise<void>;
  reset: () => void;
}

export const useEmailStore = create<EmailState>()(
  immer((set, get) => ({
    // 初始状态
    accounts: [],
    activeAccountIds: new Set(),
    currentAccountId: null,
    emails: new Map(),
    emailThreads: new Map(),
    selectedEmailIds: new Set(),
    currentEmail: null,
    currentThread: null,
    folders: new Map(),
    currentFolderId: "inbox",
    expandedFolders: new Set(),
    filter: {},
    sort: { by: "date", order: "desc" },
    searchTerm: "",
    viewMode: "list",
    labels: new Map(),
    composeDrafts: [],
    activeComposeDraftId: null,
    syncStatus: new Map(),
    isSyncing: false,
    lastSyncTime: null,
    isLoading: false,
    error: null,
    selectedPane: "list",
    isPaneCollapsed: {
      folders: false,
      list: false,
      detail: false,
    },
    paneWidths: {
      folders: 260,
      list: 400,
      detail: 0, // 剩余空间
    },

    // 账户操作
    addAccount: async (account) => {
      await storeLogger.info("Adding account", { email: account.email });
      set((state) => {
        state.accounts.push(account);
        state.activeAccountIds.add(account.id);
        state.emails.set(account.id, []);
        state.folders.set(account.id, []);
        state.labels.set(account.id, []);
        if (!state.currentAccountId || account.isDefault) {
          state.currentAccountId = account.id;
        }
      });
      await get().loadEmails(account.id);
    },

    removeAccount: async (accountId) => {
      await storeLogger.info("Removing account", { accountId });
      set((state) => {
        state.accounts = state.accounts.filter((a) => a.id !== accountId);
        state.activeAccountIds.delete(accountId);
        state.emails.delete(accountId);
        state.folders.delete(accountId);
        state.labels.delete(accountId);
        state.syncStatus.delete(accountId);
        if (state.currentAccountId === accountId) {
          state.currentAccountId = state.accounts[0]?.id || null;
        }
      });
    },

    updateAccount: (accountId, updates) => {
      set((state) => {
        const index = state.accounts.findIndex((a) => a.id === accountId);
        if (index !== -1) {
          Object.assign(state.accounts[index], updates);
        }
      });
    },

    setCurrentAccount: (accountId) => {
      set((state) => {
        state.currentAccountId = accountId;
      });
    },

    toggleAccountActive: (accountId) => {
      set((state) => {
        if (state.activeAccountIds.has(accountId)) {
          state.activeAccountIds.delete(accountId);
        } else {
          state.activeAccountIds.add(accountId);
        }
      });
    },

    switchAccount: async (accountId) => {
      await storeLogger.info("Switching account", { accountId });
      get().setCurrentAccount(accountId);
      const state = get();
      await state.loadEmails(accountId, state.currentFolderId);
    },

    // 邮件操作
    loadEmails: async (accountId, folderId, _force = false) => {
      const state = get();
      const targetAccountId = accountId || state.currentAccountId;
      const targetFolderId = folderId || state.currentFolderId;

      if (!targetAccountId) {
        await emailLogger.warn("No account selected");
        return;
      }

      await emailLogger.info("Loading emails", {
        accountId: targetAccountId,
        folderId: targetFolderId,
      });

      set({ isLoading: true, error: null });

      try {
        // 使用 mock 数据
        const emails = mockEmailStore.getEmailsByFolder(
          targetAccountId,
          targetFolderId as EmailFolder,
        );

        // 应用过滤器
        let filteredEmails = [...emails];
        const { filter, searchTerm, sort } = state;

        // 搜索过滤
        if (searchTerm) {
          const term = searchTerm.toLowerCase();
          filteredEmails = filteredEmails.filter(
            (email) =>
              email.subject.toLowerCase().includes(term) ||
              email.body.toLowerCase().includes(term) ||
              email.from.email.toLowerCase().includes(term) ||
              email.from.name?.toLowerCase().includes(term),
          );
        }

        // 其他过滤器
        if (filter.isRead !== undefined) {
          filteredEmails = filteredEmails.filter(
            (e) => e.isRead === filter.isRead,
          );
        }
        if (filter.isStarred !== undefined) {
          filteredEmails = filteredEmails.filter(
            (e) => e.isStarred === filter.isStarred,
          );
        }
        if (filter.hasAttachments !== undefined) {
          filteredEmails = filteredEmails.filter(
            (e) => e.hasAttachments === filter.hasAttachments,
          );
        }

        // 排序
        filteredEmails.sort((a, b) => {
          let comparison = 0;
          switch (sort.by) {
            case "date":
              comparison =
                new Date(b.date).getTime() - new Date(a.date).getTime();
              break;
            case "subject":
              comparison = a.subject.localeCompare(b.subject);
              break;
            case "from":
              comparison = (a.from.name || a.from.email).localeCompare(
                b.from.name || b.from.email,
              );
              break;
            case "size":
              comparison = (b.size || 0) - (a.size || 0);
              break;
            case "importance":
              comparison = Number(b.isImportant) - Number(a.isImportant);
              break;
          }
          return sort.order === "asc" ? -comparison : comparison;
        });

        set((state) => {
          state.emails.set(targetAccountId, filteredEmails);
          state.isLoading = false;
        });

        await emailLogger.info(`Loaded ${filteredEmails.length} emails`);
      } catch (error) {
        await emailLogger.error("Failed to load emails", error);
        set({
          error:
            error instanceof Error ? error.message : "Failed to load emails",
          isLoading: false,
        });
      }
    },

    loadEmailsForAllAccounts: async () => {
      const { activeAccountIds, currentFolderId } = get();
      await Promise.all(
        Array.from(activeAccountIds).map((accountId) =>
          get().loadEmails(accountId, currentFolderId),
        ),
      );
    },

    refreshCurrentFolder: async () => {
      const { currentAccountId, currentFolderId } = get();
      if (currentAccountId) {
        await get().loadEmails(currentAccountId, currentFolderId, true);
      }
    },

    selectEmail: (email) => {
      set((state) => {
        state.currentEmail = email;
        state.selectedEmailIds.clear();
        state.selectedEmailIds.add(email.id);
      });
      if (!email.isRead) {
        get().markAsRead([email.id]);
      }
    },

    selectThread: (thread) => {
      set((state) => {
        state.currentThread = thread;
        state.selectedEmailIds.clear();
        thread.emails.forEach((email) => {
          state.selectedEmailIds.add(email.id);
        });
      });
    },

    toggleEmailSelection: (emailId) => {
      set((state) => {
        if (state.selectedEmailIds.has(emailId)) {
          state.selectedEmailIds.delete(emailId);
        } else {
          state.selectedEmailIds.add(emailId);
        }
      });
    },

    selectAllEmails: () => {
      const { currentAccountId, emails } = get();
      if (currentAccountId) {
        const accountEmails = emails.get(currentAccountId) || [];
        set((state) => {
          state.selectedEmailIds = new Set(accountEmails.map((e) => e.id));
        });
      }
    },

    clearSelection: () => {
      set((state) => {
        state.selectedEmailIds.clear();
      });
    },

    // 邮件动作
    markAsRead: async (emailIds) => {
      await emailLogger.info("Marking emails as read", {
        count: emailIds.length,
      });
      set((state) => {
        state.emails.forEach((accountEmails) => {
          accountEmails.forEach((email) => {
            if (emailIds.includes(email.id)) {
              email.isRead = true;
            }
          });
        });
      });
      await get().refreshFolders();
    },

    markAsUnread: async (emailIds) => {
      await emailLogger.info("Marking emails as unread", {
        count: emailIds.length,
      });
      set((state) => {
        state.emails.forEach((accountEmails) => {
          accountEmails.forEach((email) => {
            if (emailIds.includes(email.id)) {
              email.isRead = false;
            }
          });
        });
      });
      await get().refreshFolders();
    },

    toggleStar: async (emailId) => {
      set((state) => {
        state.emails.forEach((accountEmails) => {
          const email = accountEmails.find((e) => e.id === emailId);
          if (email) {
            email.isStarred = !email.isStarred;
          }
        });
      });
    },

    toggleFlag: async (emailId) => {
      set((state) => {
        state.emails.forEach((accountEmails) => {
          const email = accountEmails.find((e) => e.id === emailId);
          if (email) {
            email.isFlagged = !email.isFlagged;
          }
        });
      });
    },

    toggleImportant: async (emailId) => {
      set((state) => {
        state.emails.forEach((accountEmails) => {
          const email = accountEmails.find((e) => e.id === emailId);
          if (email) {
            email.isImportant = !email.isImportant;
          }
        });
      });
    },

    deleteEmails: async (emailIds) => {
      await emailLogger.info("Moving emails to trash", {
        count: emailIds.length,
      });
      set((state) => {
        state.emails.forEach((accountEmails) => {
          accountEmails.forEach((email) => {
            if (emailIds.includes(email.id)) {
              email.folder = "trash";
            }
          });
        });
      });
      await get().refreshFolders();
    },

    permanentlyDeleteEmails: async (emailIds) => {
      await emailLogger.info("Permanently deleting emails", {
        count: emailIds.length,
      });
      set((state) => {
        state.emails.forEach((accountEmails, accountId) => {
          const filtered = accountEmails.filter(
            (email) => !emailIds.includes(email.id),
          );
          state.emails.set(accountId, filtered);
        });
      });
      await get().refreshFolders();
    },

    moveToFolder: async (emailIds, folderId) => {
      await emailLogger.info("Moving emails to folder", {
        count: emailIds.length,
        folder: folderId,
      });
      set((state) => {
        state.emails.forEach((accountEmails) => {
          accountEmails.forEach((email) => {
            if (emailIds.includes(email.id)) {
              email.folder = folderId as EmailFolder;
            }
          });
        });
      });
      await get().refreshFolders();
    },

    archiveEmails: async (emailIds) => {
      await get().moveToFolder(emailIds, "archive");
    },

    markAsSpam: async (emailIds) => {
      await get().moveToFolder(emailIds, "spam");
    },

    // 文件夹操作
    selectFolder: async (folderId) => {
      await storeLogger.info("Selecting folder", { folderId });
      set((state) => {
        state.currentFolderId = folderId;
        state.selectedEmailIds.clear();
      });
      const state = get();
      if (state.activeAccountIds.size > 1) {
        // 如果有多个活动账户，加载所有账户的该文件夹
        await state.loadEmailsForAllAccounts();
      } else if (state.currentAccountId) {
        await state.loadEmails(state.currentAccountId, folderId);
      }
    },

    createFolder: async (accountId, name, parentId) => {
      await storeLogger.info("Creating folder", { accountId, name, parentId });
      const newFolder: FolderInfo = {
        id: uuidv4(),
        name,
        type: name.toLowerCase() as EmailFolder,
        count: 0,
        unreadCount: 0,
        accountId,
        parentId,
        isSystem: false,
      };

      set((state) => {
        const folders = state.folders.get(accountId) || [];
        folders.push(newFolder);
        state.folders.set(accountId, folders);
      });
    },

    renameFolder: async (folderId, newName) => {
      set((state) => {
        state.folders.forEach((folders) => {
          const folder = folders.find((f) => f.id === folderId);
          if (folder) {
            folder.name = newName;
          }
        });
      });
    },

    deleteFolder: async (folderId) => {
      set((state) => {
        state.folders.forEach((folders, accountId) => {
          const filtered = folders.filter((f) => f.id !== folderId);
          state.folders.set(accountId, filtered);
        });
      });
    },

    toggleFolderExpanded: (folderId) => {
      set((state) => {
        if (state.expandedFolders.has(folderId)) {
          state.expandedFolders.delete(folderId);
        } else {
          state.expandedFolders.add(folderId);
        }
      });
    },

    refreshFolders: async (accountId) => {
      const state = get();
      const targetAccountId = accountId || state.currentAccountId;
      if (targetAccountId) {
        const folders = mockEmailStore.getFolderInfo(targetAccountId);
        set((draft) => {
          draft.folders.set(targetAccountId, folders);
        });
      }
    },

    // 标签操作
    createLabel: async (accountId, name, color) => {
      const newLabel: Label = {
        id: uuidv4(),
        name,
        color,
        accountId,
        isSystemLabel: false,
      };

      set((state) => {
        const labels = state.labels.get(accountId) || [];
        labels.push(newLabel);
        state.labels.set(accountId, labels);
      });
    },

    updateLabel: async (labelId, updates) => {
      set((state) => {
        state.labels.forEach((labels) => {
          const label = labels.find((l) => l.id === labelId);
          if (label) {
            Object.assign(label, updates);
          }
        });
      });
    },

    deleteLabel: async (labelId) => {
      set((state) => {
        state.labels.forEach((labels, accountId) => {
          const filtered = labels.filter((l) => l.id !== labelId);
          state.labels.set(accountId, filtered);
        });
      });
    },

    addLabelToEmails: async (emailIds, labelId) => {
      set((state) => {
        const label = Array.from(state.labels.values())
          .flat()
          .find((l) => l.id === labelId);
        if (label) {
          state.emails.forEach((accountEmails) => {
            accountEmails.forEach((email) => {
              if (emailIds.includes(email.id)) {
                if (!email.labels) email.labels = [];
                if (!email.labels.find((l) => l.id === labelId)) {
                  email.labels.push(label);
                }
              }
            });
          });
        }
      });
    },

    removeLabelFromEmails: async (emailIds, labelId) => {
      set((state) => {
        state.emails.forEach((accountEmails) => {
          accountEmails.forEach((email) => {
            if (emailIds.includes(email.id) && email.labels) {
              email.labels = email.labels.filter((l) => l.id !== labelId);
            }
          });
        });
      });
    },

    // 搜索和过滤
    setSearchTerm: (term) => {
      set({ searchTerm: term });
      get().refreshCurrentFolder();
    },

    setFilter: (filter) => {
      set((state) => {
        state.filter = { ...state.filter, ...filter };
      });
      get().refreshCurrentFolder();
    },

    setSort: (sort) => {
      set({ sort });
      get().refreshCurrentFolder();
    },

    clearFilters: () => {
      set({ filter: {}, searchTerm: "" });
      get().refreshCurrentFolder();
    },

    setViewMode: (mode) => {
      set({ viewMode: mode });
    },

    // 撰写邮件
    createDraft: (draft) => {
      const { currentAccountId } = get();
      if (!currentAccountId) return "";

      const draftId = uuidv4();
      const newDraft: ComposeDraft = {
        id: draftId,
        accountId: currentAccountId,
        to: [],
        subject: "",
        body: "",
        isDraft: true,
        ...draft,
      };

      set((state) => {
        state.composeDrafts.push(newDraft);
        state.activeComposeDraftId = draftId;
      });

      return draftId;
    },

    updateDraft: (draftId, updates) => {
      set((state) => {
        const draft = state.composeDrafts.find((d) => d.id === draftId);
        if (draft) {
          Object.assign(draft, updates);
        }
      });
    },

    deleteDraft: (draftId) => {
      set((state) => {
        state.composeDrafts = state.composeDrafts.filter(
          (d) => d.id !== draftId,
        );
        if (state.activeComposeDraftId === draftId) {
          state.activeComposeDraftId = null;
        }
      });
    },

    sendEmail: async (draftId) => {
      await emailLogger.info("Sending email", { draftId });
      // Mock sending
      await new Promise((resolve) => setTimeout(resolve, 1000));
      get().deleteDraft(draftId);
    },

    saveDraft: async (draftId) => {
      await emailLogger.info("Saving draft", { draftId });
      // Mock saving
      await new Promise((resolve) => setTimeout(resolve, 500));
    },

    setActiveDraft: (draftId) => {
      set({ activeComposeDraftId: draftId });
    },

    // 同步操作
    syncAccount: async (accountId) => {
      await storeLogger.info("Syncing account", { accountId });
      set((state) => {
        state.syncStatus.set(accountId, {
          accountId,
          folder: "inbox",
          isSync: true,
          progress: 0,
        });
      });

      // Mock sync
      for (let i = 0; i <= 100; i += 10) {
        await new Promise((resolve) => setTimeout(resolve, 100));
        set((state) => {
          const status = state.syncStatus.get(accountId);
          if (status) {
            status.progress = i;
          }
        });
      }

      set((state) => {
        state.syncStatus.set(accountId, {
          accountId,
          folder: "inbox",
          isSync: false,
          lastSyncTime: new Date(),
          newEmails: Math.floor(Math.random() * 10),
        });
      });

      await get().loadEmails(accountId);
    },

    syncFolder: async (accountId, folderId) => {
      await storeLogger.info("Syncing folder", { accountId, folderId });
      // Implementation similar to syncAccount but for specific folder
    },

    syncAllAccounts: async () => {
      const state = get();
      set({ isSyncing: true });
      await Promise.all(
        Array.from(state.activeAccountIds).map((accountId) =>
          state.syncAccount(accountId),
        ),
      );
      set({ isSyncing: false, lastSyncTime: new Date() });
    },

    stopSync: () => {
      set({ isSyncing: false });
      set((state) => {
        state.syncStatus.forEach((status) => {
          status.isSync = false;
        });
      });
    },

    // UI 操作
    setPaneWidth: (pane, width) => {
      set((state) => {
        state.paneWidths[pane] = width;
      });
    },

    togglePaneCollapse: (pane) => {
      set((state) => {
        state.isPaneCollapsed[pane] = !state.isPaneCollapsed[pane];
      });
    },

    setSelectedPane: (pane) => {
      set({ selectedPane: pane });
    },

    // 初始化
    initialize: async () => {
      await storeLogger.info("Initializing email store");
      set({ isLoading: true });

      try {
        // 加载账户
        const accounts = mockEmailStore.getAccounts();

        set((state) => {
          state.accounts = accounts;
          accounts.forEach((account) => {
            state.activeAccountIds.add(account.id);
            state.emails.set(account.id, []);
            state.folders.set(account.id, []);
            state.labels.set(account.id, []);
          });
        });

        // 设置默认账户
        const defaultAccount = accounts.find((a) => a.isDefault) || accounts[0];
        if (defaultAccount) {
          set({ currentAccountId: defaultAccount.id });

          // 加载文件夹信息
          for (const account of accounts) {
            const folders = mockEmailStore.getFolderInfo(account.id);
            set((state) => {
              state.folders.set(account.id, folders);
            });
          }

          // 加载默认账户的收件箱
          await get().loadEmails(defaultAccount.id, "inbox");
        }

        set({ isLoading: false });
        await storeLogger.info("Email store initialized successfully");
      } catch (error) {
        await storeLogger.error("Failed to initialize email store", error);
        set({
          error:
            error instanceof Error ? error.message : "Failed to initialize",
          isLoading: false,
        });
      }
    },

    reset: () => {
      set({
        accounts: [],
        activeAccountIds: new Set(),
        currentAccountId: null,
        emails: new Map(),
        emailThreads: new Map(),
        selectedEmailIds: new Set(),
        currentEmail: null,
        currentThread: null,
        folders: new Map(),
        currentFolderId: "inbox",
        expandedFolders: new Set(),
        filter: {},
        sort: { by: "date", order: "desc" },
        searchTerm: "",
        viewMode: "list",
        labels: new Map(),
        composeDrafts: [],
        activeComposeDraftId: null,
        syncStatus: new Map(),
        isSyncing: false,
        lastSyncTime: null,
        isLoading: false,
        error: null,
      });
    },
  })),
);
