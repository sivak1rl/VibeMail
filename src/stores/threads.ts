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
  fetchThreads: (accountId: string, focusOnly?: boolean) => Promise<void>;
  loadMoreThreads: (accountId: string) => Promise<void>;
  selectThread: (threadId: string) => Promise<void>;
  syncAccount: (accountId: string) => Promise<SyncResult>;
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

  fetchThreads: async (accountId, focusOnly = false) => {
    set({ loading: true });
    try {
      const PAGE = 50;
      const threads = await invoke<Thread[]>("list_threads", {
        request: {
          account_id: accountId,
          limit: PAGE,
          offset: 0,
          focus_only: focusOnly,
        },
      });
      set({ threads, loading: false, hasMore: threads.length >= PAGE });
    } catch (e) {
      set({ loading: false });
    }
  },

  loadMoreThreads: async (accountId) => {
    const { threads, loading, hasMore, focusMode } = get();
    if (loading || !hasMore) return;
    set({ loading: true });
    try {
      const PAGE = 50;
      const more = await invoke<Thread[]>("list_threads", {
        request: {
          account_id: accountId,
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

  syncAccount: async (accountId) => {
    set({ syncing: true, syncError: null, syncProgress: "Starting sync…" });
    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<string>("sync-progress", (event) => {
        set({ syncProgress: event.payload });
      });
      const result = await invoke<SyncResult>("sync_account", { accountId });
      set({ syncing: false, syncError: result.error, syncProgress: null });
      if (!result.error) {
        await get().fetchThreads(accountId, get().focusMode);
      }
      return result;
    } catch (e) {
      const error = String(e);
      set({ syncing: false, syncError: error, syncProgress: null });
      throw e;
    } finally {
      if (unlisten) unlisten();
    }
  },

  setFocusMode: (v) => set({ focusMode: v }),
}));
