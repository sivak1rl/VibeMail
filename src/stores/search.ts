import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { Thread } from "./threads";

interface SearchStore {
  query: string;
  results: Thread[];
  history: string[];
  searching: boolean;
  hasMore: boolean;
  lastSearchMode: "fts" | "semantic";
  reindexing: boolean;
  reindexProgress: string | null;
  setQuery: (q: string) => void;
  search: (query: string, accountId: string, mailboxId?: string | null) => Promise<void>;
  searchSemantic: (
    query: string,
    accountId: string,
    mailboxId?: string | null,
  ) => Promise<void>;
  loadMore: (accountId: string, mailboxId?: string | null) => Promise<void>;
  reindexAll: (accountId: string, force?: boolean) => Promise<void>;
  clear: () => void;
  loadHistory: () => void;
  addToHistory: (query: string) => void;
  clearHistory: () => void;
}

const HISTORY_KEY = "vibemail.search_history";
const PAGE_SIZE = 30;

export const useSearchStore = create<SearchStore>((set, get) => ({
  query: "",
  results: [],
  history: [],
  searching: false,
  hasMore: false,
  lastSearchMode: "fts",
  reindexing: false,
  reindexProgress: null,

  setQuery: (q) => set({ query: q }),

  loadHistory: () => {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (raw) {
      try {
        set({ history: JSON.parse(raw) });
      } catch {
        set({ history: [] });
      }
    }
  },

  clearHistory: () => {
    localStorage.removeItem(HISTORY_KEY);
    set({ history: [] });
  },

  addToHistory: (query: string) => {
    if (!query.trim()) return;
    const history = [query, ...get().history.filter((q) => q !== query)].slice(0, 10);
    set({ history });
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
  },

  search: async (query, accountId, mailboxId = null) => {
    if (!query.trim()) {
      set({ results: [], query: "", hasMore: false });
      return;
    }
    set({ searching: true, query, lastSearchMode: "fts" });
    try {
      const results = await invoke<Thread[]>("search_messages", {
        request: { query, account_id: accountId, mailbox_id: mailboxId, limit: PAGE_SIZE },
      });
      set({ results, searching: false, hasMore: results.length >= PAGE_SIZE });
      get().addToHistory(query);
    } catch {
      set({ searching: false });
    }
  },

  searchSemantic: async (query, accountId, mailboxId = null) => {
    if (!query.trim()) {
      set({ results: [], query: "", hasMore: false });
      return;
    }
    set({ searching: true, query, lastSearchMode: "semantic" });
    try {
      const results = await invoke<Thread[]>("search_semantic", {
        request: { query, account_id: accountId, mailbox_id: mailboxId, limit: PAGE_SIZE },
      });
      set({ results, searching: false, hasMore: results.length >= PAGE_SIZE });
      get().addToHistory(query);
    } catch {
      set({ searching: false });
    }
  },

  loadMore: async (accountId, mailboxId = null) => {
    const { query, results, searching, hasMore, lastSearchMode } = get();
    if (searching || !hasMore || !query) return;

    set({ searching: true });
    try {
      const command = lastSearchMode === "semantic" ? "search_semantic" : "search_messages";
      const more = await invoke<Thread[]>(command, {
        request: {
          query,
          account_id: accountId,
          mailbox_id: mailboxId,
          limit: PAGE_SIZE,
          offset: results.length,
        },
      });
      set({
        results: [...results, ...more],
        searching: false,
        hasMore: more.length >= PAGE_SIZE,
      });
    } catch {
      set({ searching: false });
    }
  },

  reindexAll: async (accountId, force = true) => {
    set({ reindexing: true, reindexProgress: "Starting background reindexing…" });
    let unlisten: UnlistenFn | null = null;

    try {
      unlisten = await listen<string>("reindex-progress", (event) => {
        set({ reindexProgress: event.payload });
      });

      if (force) {
        await invoke<void>("reindex_all_semantic", { accountId });
      }

      const checkStatus = async () => {
        const isReindexing = await invoke<boolean>("get_reindex_status", { accountId });
        if (!isReindexing) {
          if (pollTimer) clearInterval(pollTimer);
          if (unlisten) unlisten();
          set({ reindexing: false, reindexProgress: null });
        }
      };

      const pollTimer = setInterval(() => {
        void checkStatus();
      }, 2000);
    } catch (e) {
      // If we are just attaching to an existing sync, ignore "already in progress" error
      if (String(e).includes("already in progress")) {
        // Fall through to polling
      } else {
        if (unlisten) unlisten();
        set({ reindexing: false, reindexProgress: null });
        throw e;
      }
    }
  },

  clear: () => set({ query: "", results: [], hasMore: false }),
}));
