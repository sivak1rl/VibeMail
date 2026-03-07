import { create } from "zustand";

const STORAGE_KEY = "vibemail.preferences";
const DEFAULT_AUTO_SYNC_MINUTES = 15;
const DEFAULT_AUTO_LABEL_NEW_EMAILS = false;

export interface CustomCategoryPreference {
  name: string;
  examples: string[];
}

interface PreferencesState {
  autoSyncIntervalMinutes: number;
  autoLabelNewEmails: boolean;
  customCategories: CustomCategoryPreference[];
  setAutoSyncIntervalMinutes: (minutes: number) => void;
  setAutoLabelNewEmails: (enabled: boolean) => void;
  setCustomCategories: (categories: CustomCategoryPreference[]) => void;
}

interface StoredPreferences {
  autoSyncIntervalMinutes?: number;
  autoLabelNewEmails?: boolean;
  customCategories?: CustomCategoryPreference[];
}

function loadPreferences(): StoredPreferences {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    return (JSON.parse(raw) as StoredPreferences) ?? {};
  } catch {
    return {};
  }
}

function loadAutoSyncMinutes(): number {
  const parsed = loadPreferences();
  const value = parsed.autoSyncIntervalMinutes;
  if (typeof value !== "number" || Number.isNaN(value)) {
    return DEFAULT_AUTO_SYNC_MINUTES;
  }
  return Math.max(0, Math.floor(value));
}

function loadAutoLabelNewEmails(): boolean {
  const parsed = loadPreferences();
  if (typeof parsed.autoLabelNewEmails !== "boolean") {
    return DEFAULT_AUTO_LABEL_NEW_EMAILS;
  }
  return parsed.autoLabelNewEmails;
}

function loadCustomCategories(): CustomCategoryPreference[] {
  const parsed = loadPreferences();
  if (!Array.isArray(parsed.customCategories)) return [];
  return parsed.customCategories
    .map((item) => {
      const name =
        typeof item?.name === "string"
          ? item.name.trim().slice(0, 32)
          : "";
      const examples = Array.isArray(item?.examples)
        ? item.examples
            .filter((example): example is string => typeof example === "string")
            .map((example) => example.trim().slice(0, 120))
            .filter(Boolean)
            .slice(0, 6)
        : [];
      if (!name) return null;
      return { name, examples };
    })
    .filter((item): item is CustomCategoryPreference => item !== null)
    .slice(0, 12);
}

function persistPreferences(next: StoredPreferences): void {
  if (typeof window === "undefined") return;
  try {
    const current = loadPreferences();
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...current, ...next }));
  } catch {
    // Ignore localStorage write failures.
  }
}

export const usePreferencesStore = create<PreferencesState>((set) => ({
  autoSyncIntervalMinutes: loadAutoSyncMinutes(),
  autoLabelNewEmails: loadAutoLabelNewEmails(),
  customCategories: loadCustomCategories(),
  setAutoSyncIntervalMinutes: (minutes) => {
    const normalized = Number.isFinite(minutes)
      ? Math.max(0, Math.floor(minutes))
      : DEFAULT_AUTO_SYNC_MINUTES;
    persistPreferences({ autoSyncIntervalMinutes: normalized });
    set({ autoSyncIntervalMinutes: normalized });
  },
  setAutoLabelNewEmails: (enabled) => {
    persistPreferences({ autoLabelNewEmails: enabled });
    set({ autoLabelNewEmails: enabled });
  },
  setCustomCategories: (categories) => {
    const normalized = categories
      .map((item) => ({
        name: item.name.trim().slice(0, 32),
        examples: item.examples
          .map((example) => example.trim().slice(0, 120))
          .filter(Boolean)
          .slice(0, 6),
      }))
      .filter((item) => item.name.length > 0)
      .slice(0, 12);
    persistPreferences({ customCategories: normalized });
    set({ customCategories: normalized });
  },
}));
