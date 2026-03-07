import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";
import type { Thread } from "./threads";

interface SearchStore {
  query: string;
  results: Thread[];
  searching: boolean;
  reindexing: boolean;
  reindexProgress: string | null;
  setQuery: (q: string) => void;
  search: (query: string, accountId: string, mailboxId?: string | null) => Promise<void>;
  searchSemantic: (
    query: string,
    accountId: string,
    mailboxId?: string | null,
  ) => Promise<void>;
  reindexAll: (accountId: string) => Promise<void>;
  clear: () => void;
}

export const useSearchStore = create<SearchStore>((set) => ({
  query: "",
  results: [],
  searching: false,
  reindexing: false,
  reindexProgress: null,

  setQuery: (q) => set({ query: q }),

  search: async (query, accountId, mailboxId = null) => {
    if (!query.trim()) {
      set({ results: [], query: "" });
      return;
    }
    set({ searching: true, query });
    try {
      const results = await invoke<Thread[]>("search_messages", {
        request: { query, account_id: accountId, mailbox_id: mailboxId, limit: 30 },
      });
      set({ results, searching: false });
    } catch (e) {
      set({ searching: false });
    }
  },

  searchSemantic: async (query, accountId, mailboxId = null) => {
    if (!query.trim()) {
      set({ results: [], query: "" });
      return;
    }
    set({ searching: true, query });
    try {
      const results = await invoke<Thread[]>("search_semantic", {
        request: { query, account_id: accountId, mailbox_id: mailboxId, limit: 30 },
      });
      set({ results, searching: false });
    } catch (e) {
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
        set({ reindexing: false, reindexProgress: null });
        throw e;
      }
    }
  },

  clear: () => set({ query: "", results: [] }),
}));
