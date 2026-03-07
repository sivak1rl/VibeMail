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
    set({ selectedThreadId: threadId });
    try {
      const messages = await invoke<Message[]>("get_thread", { threadId });
      set({ threadMessages: messages });
    } catch (e) {
      console.error("Failed to load thread messages:", e);
    }
  },

  syncAccount: async (accountId, mailboxId = null) => {
    set({ syncing: true, syncError: null, syncProgress: "Starting sync…" });
    let unlisten: UnlistenFn | null = null;
    let pollTimer: ReturnType<typeof setInterval> | null = null;
    let pollInFlight = false;

    const refreshWhileSyncing = async () => {
      if (pollInFlight) return;
      pollInFlight = true;
      try {
        const PAGE = 50;
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

        const { selectedThreadId } = get();
        const nextSelectedId =
          selectedThreadId && threads.some((thread) => thread.id === selectedThreadId)
            ? selectedThreadId
            : threads[0]?.id ?? null;

        set({
          threads,
          hasMore: threads.length >= PAGE,
          selectedThreadId: nextSelectedId,
        });

        if (nextSelectedId && nextSelectedId !== selectedThreadId) {
          await get().selectThread(nextSelectedId);
        }
      } catch {
        // Keep sync running even if a mid-sync refresh fails.
      } finally {
        pollInFlight = false;
      }
    };

    try {
      unlisten = await listen<string>("sync-progress", (event) => {
        set({ syncProgress: event.payload });
      });

      // Render messages incrementally as batches are persisted during sync.
      await refreshWhileSyncing();
      pollTimer = setInterval(() => {
        void refreshWhileSyncing();
      }, 900);

      const result = await invoke<SyncResult>("sync_account", {
        request: {
          account_id: accountId,
          mailbox_id: mailboxId,
        },
      });
      if (pollTimer) {
        clearInterval(pollTimer);
        pollTimer = null;
      }

      await refreshWhileSyncing();
      set({ syncing: false, syncError: result.error, syncProgress: null });
      return result;
    } catch (e) {
      const error = String(e);
      set({ syncing: false, syncError: error, syncProgress: null });
      throw e;
    } finally {
      if (pollTimer) clearInterval(pollTimer);
      if (unlisten) unlisten();
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

  setFocusMode: (v) => set({ focusMode: v }),
}));
