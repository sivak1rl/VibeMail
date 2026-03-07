import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";

export interface EmailAddress {
  name: string | null;
  email: string;
}

export interface Message {
  id: string;
  account_id: string;
  mailbox_id: string;
  uid: number;
  message_id: string | null;
  thread_id: string | null;
  subject: string | null;
  from: EmailAddress[];
  to: EmailAddress[];
  cc: EmailAddress[];
  date: string | null;
  body_text: string | null;
  body_html: string | null;
  references_ids: string[];
  in_reply_to: string | null;
  flags: string[];
  has_attachments: boolean;
  triage_score: number | null;
  ai_summary: string | null;
}

export interface Thread {
  id: string;
  account_id: string;
  subject: string | null;
  participants: EmailAddress[];
  message_count: number;
  unread_count: number;
  is_flagged: boolean;
  has_attachments: boolean;
  last_date: string | null;
  last_from: string | null;
  triage_score: number | null;
  labels: string[];
  messages?: Message[];
}

interface SyncResult {
  account_id: string;
  mailbox_id: string | null;
  new_messages: number;
  error: string | null;
}

interface ThreadStore {
  threads: Thread[];
  selectedThreadId: string | null;
  threadMessages: Message[];
  loading: boolean;
  syncing: boolean;
  syncError: string | null;
  syncProgress: string | null;
  focusMode: boolean;
  hasMore: boolean;
  fetchThreads: (accountId: string, mailboxId?: string | null, focusOnly?: boolean) => Promise<void>;
  loadMoreThreads: (accountId: string, mailboxId?: string | null) => Promise<void>;
  selectThread: (threadId: string) => Promise<void>;
  syncAccount: (accountId: string, mailboxId?: string | null) => Promise<SyncResult>;
  setThreadsRead: (threadIds: string[], read: boolean) => Promise<void>;
  setThreadsFlagged: (threadIds: string[], flagged: boolean) => Promise<void>;
  archiveThreads: (threadIds: string[]) => Promise<void>;
  fetchHistory: (accountId: string, mailboxId: string | null, days?: number, limit?: number) => Promise<void>;
  applyThreadLabels: (
    labelsByThread: Record<string, string>,
    knownCategoryLabels?: string[],
  ) => void;
  setFocusMode: (v: boolean) => void;
}

export const useThreadStore = create<ThreadStore>((set, get) => ({
  threads: [],
  selectedThreadId: null,
  threadMessages: [],
  loading: false,
  syncing: false,
  syncError: null,
  syncProgress: null,
  focusMode: false,
  hasMore: true,

  fetchThreads: async (accountId, mailboxId = null, focusOnly = false) => {
    set({ loading: true, selectedThreadId: null, threadMessages: [] });
    try {
      const PAGE = 50;
      const threads = await invoke<Thread[]>("list_threads", {
        request: {
          account_id: accountId,
          mailbox_id: mailboxId,
          limit: PAGE,
          offset: 0,
          focus_only: focusOnly,
        },
      });
      set({
        threads,
        loading: false,
        hasMore: threads.length >= PAGE,
        selectedThreadId: threads[0]?.id ?? null,
      });

      if (threads[0]?.id) {
        await get().selectThread(threads[0].id);
      }
    } catch (e) {
      set({ loading: false });
    }
  },

  loadMoreThreads: async (accountId, mailboxId = null) => {
    const { threads, loading, hasMore, focusMode } = get();
    if (loading || !hasMore) return;
    set({ loading: true });
    try {
      const PAGE = 50;
      const more = await invoke<Thread[]>("list_threads", {
        request: {
          account_id: accountId,
          mailbox_id: mailboxId,
          limit: PAGE,
          offset: threads.length,
          focus_only: focusMode,
        },
      });
      set({
        threads: [...threads, ...more],
        loading: false,
        hasMore: more.length >= PAGE,
      });
    } catch (e) {
      set({ loading: false });
    }
  },

  selectThread: async (threadId) => {
    if (!threadId) return;
    set({ selectedThreadId: threadId });
    try {
      const messages = await invoke<Message[]>("get_thread", { threadId });
      if (!Array.isArray(messages)) {
        throw new Error("get_thread returned invalid data (not an array)");
      }
      set({ threadMessages: messages });
    } catch (e) {
      console.error("Store: Failed to load thread messages:", e);
      set({ threadMessages: [] });
    }
  },

  syncAccount: async (accountId, mailboxId = null) => {
    set({ syncing: true, syncError: null, syncProgress: "Starting background sync…" });
    let unlisten: UnlistenFn | null = null;

    try {
      unlisten = await listen<string>("sync-progress", (event) => {
        set({ syncProgress: event.payload });
      });

      // Start the sync (returns immediately now)
      await invoke<SyncResult>("sync_account", {
        request: {
          account_id: accountId,
          mailbox_id: mailboxId,
        },
      });

      // Poll for completion
      const checkStatus = async () => {
        const isSyncing = await invoke<boolean>("get_sync_status", { accountId });
        if (!isSyncing) {
          if (pollTimer) clearInterval(pollTimer);
          set({ syncing: false, syncProgress: null });
          
          // Refresh the view
          const currentCount = get().threads.length;
          const PAGE = Math.max(50, currentCount);
          const focusOnly = get().focusMode;
          const threads = await invoke<Thread[]>("list_threads", {
            request: {
              account_id: accountId,
              mailbox_id: mailboxId,
              limit: PAGE,
              offset: 0,
              focus_only: focusOnly,
            },
          });
          set({ threads, hasMore: threads.length >= PAGE });
        }
      };

      const pollTimer = setInterval(() => {
        void checkStatus();
      }, 500);

      // We still return the promise, but it "resolves" after starting
      return { account_id: accountId, mailbox_id: mailboxId, new_messages: 0, error: null };
    } catch (e) {
      const error = String(e);
      set({ syncing: false, syncError: error, syncProgress: null });
      throw e;
    } finally {
      // We don't unlisten immediately because sync is in background.
      // But we can't keep unlisten forever easily in this pattern.
      // For now, let's keep it until it's done.
    }
  },

  fetchHistory: async (accountId, mailboxId, days = 30, limit = 100) => {
    set({ syncing: true, syncError: null, syncProgress: "Preparing history fetch…" });
    let unlisten: UnlistenFn | null = null;

    try {
      unlisten = await listen<string>("sync-progress", (event) => {
        set({ syncProgress: event.payload });
      });

      await invoke<SyncResult>("fetch_history", {
        request: {
          account_id: accountId,
          mailbox_id: mailboxId,
          days,
          limit,
        },
      });

      // Poll for completion (similar to syncAccount)
      const checkStatus = async () => {
        const isSyncing = await invoke<boolean>("get_sync_status", { accountId });
        if (!isSyncing) {
          if (pollTimer) clearInterval(pollTimer);
          set({ syncing: false, syncProgress: null });
          
          // Refresh the view and expand by the requested limit to show the new history
          const currentCount = get().threads.length;
          const PAGE = currentCount + limit;
          const focusOnly = get().focusMode;
          
          const threads = await invoke<Thread[]>("list_threads", {
            request: {
              account_id: accountId,
              mailbox_id: mailboxId,
              limit: PAGE,
              offset: 0,
              focus_only: focusOnly,
            },
          });
          set({ threads, hasMore: threads.length >= PAGE });
        }
      };

      const pollTimer = setInterval(() => {
        void checkStatus();
      }, 500);
    } catch (e) {
      set({ syncing: false, syncError: String(e), syncProgress: null });
      throw e;
    }
  },

  setThreadsRead: async (threadIds, read) => {
    const ids = [...new Set(threadIds)].filter(Boolean);
    if (ids.length === 0) return;

    await invoke<number>("set_threads_read", {
      request: {
        thread_ids: ids,
        read,
      },
    });

    set((state) => {
      const idSet = new Set(ids);
      const threads = state.threads.map((thread) =>
        idSet.has(thread.id)
          ? {
              ...thread,
              unread_count: read ? 0 : Math.max(thread.message_count, 1),
            }
          : thread,
      );

      const selected = state.selectedThreadId;
      const selectedIsTarget = selected ? idSet.has(selected) : false;
      const threadMessages = selectedIsTarget
        ? state.threadMessages.map((message) => {
            const flags = new Set(message.flags);
            if (read) {
              flags.add("\\Seen");
            } else {
              flags.delete("\\Seen");
            }
            return { ...message, flags: Array.from(flags) };
          })
        : state.threadMessages;

      return { threads, threadMessages };
    });
  },

  setThreadsFlagged: async (threadIds, flagged) => {
    const ids = [...new Set(threadIds)].filter(Boolean);
    if (ids.length === 0) return;

    try {
      await invoke<number>("set_threads_flagged", {
        request: {
          thread_ids: ids,
          flagged,
        },
      });

      set((state) => {
        const idSet = new Set(ids);
        const threads = state.threads.map((thread) =>
          idSet.has(thread.id)
            ? {
                ...thread,
                is_flagged: flagged,
              }
            : thread,
        );

        const selected = state.selectedThreadId;
        const selectedIsTarget = selected ? idSet.has(selected) : false;
        
        let threadMessages = state.threadMessages;
        if (selectedIsTarget && Array.isArray(threadMessages)) {
          threadMessages = threadMessages.map((message) => {
            const flags = new Set(message.flags || []);
            if (flagged) {
              flags.add("\\Flagged");
            } else {
              flags.delete("\\Flagged");
            }
            return { ...message, flags: Array.from(flags) };
          });
        }

        return { threads, threadMessages };
      });
    } catch (e) {
      console.error("Store: setThreadsFlagged failed:", e);
    }
  },

  archiveThreads: async (threadIds) => {
    const ids = [...new Set(threadIds)].filter(Boolean);
    if (ids.length === 0) return;

    await invoke<number>("archive_threads", {
      request: {
        thread_ids: ids,
      },
    });

    set((state) => {
      const idSet = new Set(ids);
      const threads = state.threads.filter((thread) => !idSet.has(thread.id));
      const nextSelectedId =
        state.selectedThreadId && idSet.has(state.selectedThreadId)
          ? threads[0]?.id ?? null
          : state.selectedThreadId;

      if (nextSelectedId !== state.selectedThreadId) {
        if (nextSelectedId) {
          void get().selectThread(nextSelectedId);
        } else {
          set({ threadMessages: [] });
        }
      }

      return { threads, selectedThreadId: nextSelectedId };
    });
  },

  applyThreadLabels: (labelsByThread, knownCategoryLabels = []) => {
    set((state) => {
      const categoryLabels = new Set([
        "newsletter",
        "receipt",
        "social",
        "updates",
        ...knownCategoryLabels,
      ]);
      const threads = state.threads.map((thread) => {
        const nextCategory = labelsByThread[thread.id];
        if (!nextCategory) return thread;
        const baseLabels = thread.labels.filter((label) => !categoryLabels.has(label));
        return { ...thread, labels: [...baseLabels, nextCategory] };
      });
      return { threads };
    });
  },

  setFocusMode: (v) => set({ focusMode: v }),
}));
