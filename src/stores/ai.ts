import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

export interface AiConfig {
  provider: string;
  base_url: string;
  model_triage: string;
  model_summary: string;
  model_draft: string;
  model_extract: string;
  model_embed: string;
  privacy_mode: boolean;
  enabled: boolean;
}

export interface ExtractedAction {
  kind: string;
  text: string;
  date: string | null;
  priority: string | null;
}

export interface TriageResult {
  thread_id: string;
  score: number;
}

export interface CategorizeThreadResult {
  thread_id: string;
  label: string;
}

export interface CustomCategory {
  name: string;
  examples: string[];
}

interface ThreadInsights {
  thread_id: string;
  summary: string | null;
  actions: ExtractedAction[];
}

interface AiState {
  config: AiConfig | null;
  summaryByThread: Record<string, string>;
  summaryStreaming: Record<string, boolean>;
  draftByThread: Record<string, string>;
  draftStreaming: Record<string, boolean>;
  actionsByThread: Record<string, ExtractedAction[]>;
  configLoaded: boolean;
  batchSummarizing: boolean;
  batchCategorizing: boolean;

  loadConfig: () => Promise<void>;
  saveConfig: (config: AiConfig, apiKey?: string) => Promise<void>;
  summarizeThread: (threadId: string) => Promise<void>;
  summarizeThreads: (threadIds: string[]) => Promise<void>;
  loadThreadInsights: (threadId: string) => Promise<void>;
  categorizeThreads: (
    threadIds: string[],
    customCategories?: CustomCategory[],
  ) => Promise<CategorizeThreadResult[]>;
  draftReply: (threadId: string) => Promise<string>;
  extractActions: (threadId: string) => Promise<ExtractedAction[]>;
  triageThread: (threadId: string) => Promise<TriageResult>;
}

export const useAiStore = create<AiState>((set, get) => ({
  config: null,
  summaryByThread: {},
  summaryStreaming: {},
  draftByThread: {},
  draftStreaming: {},
  actionsByThread: {},
  configLoaded: false,
  batchSummarizing: false,
  batchCategorizing: false,

  loadConfig: async () => {
    try {
      const config = await invoke<AiConfig>("get_ai_config");
      set({ config, configLoaded: true });
    } catch (e) {
      set({ configLoaded: true });
    }
  },

  saveConfig: async (config, apiKey) => {
    await invoke("set_ai_config", { request: { config, api_key: apiKey ?? null } });
    set({ config });
  },

  summarizeThread: async (threadId) => {
    set((s) => ({
      summaryByThread: { ...s.summaryByThread, [threadId]: "" },
      summaryStreaming: { ...s.summaryStreaming, [threadId]: true },
    }));

    const eventName = `ai_summary_${threadId}`;
    const unlistenToken = await listen<string>(eventName, (event) => {
      set((s) => ({
        summaryByThread: {
          ...s.summaryByThread,
          [threadId]: (s.summaryByThread[threadId] ?? "") + event.payload,
        },
      }));
    });

    const unlistenDone = await listen<string>(`${eventName}_done`, () => {
      set((s) => ({
        summaryStreaming: { ...s.summaryStreaming, [threadId]: false },
      }));
      unlistenToken();
      unlistenDone();
    });

    try {
      await invoke("summarize_thread", {
        request: { thread_id: threadId, account_id: "" },
      });
    } catch (e) {
      set((s) => ({
        summaryStreaming: { ...s.summaryStreaming, [threadId]: false },
        summaryByThread: {
          ...s.summaryByThread,
          [threadId]: `Error: ${e}`,
        },
      }));
      unlistenToken();
      unlistenDone();
    }
  },

  summarizeThreads: async (threadIds) => {
    const ids = [...new Set(threadIds)].filter(Boolean);
    if (ids.length === 0) return;
    set({ batchSummarizing: true });
    try {
      for (const threadId of ids) {
        await get().summarizeThread(threadId);
      }
    } finally {
      set({ batchSummarizing: false });
    }
  },

  loadThreadInsights: async (threadId) => {
    const insights = await invoke<ThreadInsights>("get_thread_insights", {
      request: { thread_id: threadId, account_id: "" },
    });
    set((s) => ({
      summaryByThread:
        insights.summary !== null
          ? { ...s.summaryByThread, [threadId]: insights.summary }
          : s.summaryByThread,
      actionsByThread: { ...s.actionsByThread, [threadId]: insights.actions },
    }));
  },

  categorizeThreads: async (threadIds, customCategories) => {
    const ids = [...new Set(threadIds)].filter(Boolean);
    if (ids.length === 0) return [];
    set({ batchCategorizing: true });
    try {
      return await invoke<CategorizeThreadResult[]>("categorize_threads", {
        request: {
          thread_ids: ids,
          custom_categories: customCategories ?? [],
        },
      });
    } finally {
      set({ batchCategorizing: false });
    }
  },

  draftReply: async (threadId) => {
    set((s) => ({
      draftByThread: { ...s.draftByThread, [threadId]: "" },
      draftStreaming: { ...s.draftStreaming, [threadId]: true },
    }));

    const eventName = `ai_draft_${threadId}`;
    const unlistenToken = await listen<string>(eventName, (event) => {
      set((s) => ({
        draftByThread: {
          ...s.draftByThread,
          [threadId]: (s.draftByThread[threadId] ?? "") + event.payload,
        },
      }));
    });

    const unlistenDone = await listen<string>(`${eventName}_done`, (event) => {
      set((s) => ({
        draftStreaming: { ...s.draftStreaming, [threadId]: false },
        draftByThread: { ...s.draftByThread, [threadId]: event.payload },
      }));
      unlistenToken();
      unlistenDone();
    });

    try {
      await invoke("draft_reply", {
        request: { thread_id: threadId, account_id: "" },
      });
    } catch (e) {
      set((s) => ({
        draftStreaming: { ...s.draftStreaming, [threadId]: false },
      }));
      unlistenToken();
      unlistenDone();
      throw e;
    }

    return get().draftByThread[threadId] ?? "";
  },

  extractActions: async (threadId) => {
    const actions = await invoke<ExtractedAction[]>("extract_actions", {
      request: { thread_id: threadId, account_id: "" },
    });
    set((s) => ({ actionsByThread: { ...s.actionsByThread, [threadId]: actions } }));
    return actions;
  },

  triageThread: async (threadId) => {
    return invoke<TriageResult>("triage_thread", {
      request: { thread_id: threadId, account_id: "" },
    });
  },
}));
