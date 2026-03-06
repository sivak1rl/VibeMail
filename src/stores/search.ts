import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import type { Thread } from "./threads";

interface SearchStore {
  query: string;
  results: Thread[];
  searching: boolean;
  setQuery: (q: string) => void;
  search: (query: string, accountId: string) => Promise<void>;
  clear: () => void;
}

export const useSearchStore = create<SearchStore>((set) => ({
  query: "",
  results: [],
  searching: false,

  setQuery: (q) => set({ query: q }),

  search: async (query, accountId) => {
    if (!query.trim()) {
      set({ results: [], query: "" });
      return;
    }
    set({ searching: true, query });
    try {
      const results = await invoke<Thread[]>("search_messages", {
        request: { query, account_id: accountId, limit: 30 },
      });
      set({ results, searching: false });
    } catch (e) {
      set({ searching: false });
    }
  },

  clear: () => set({ query: "", results: [] }),
}));
