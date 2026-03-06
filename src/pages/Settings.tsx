import { useEffect, useState } from "react";
import { useAiStore, type AiConfig } from "../stores/ai";
import { useAccountStore } from "../stores/accounts";
import styles from "./Settings.module.css";

interface Props {
  onBack: () => void;
}

const PROVIDERS = [
  { value: "ollama", label: "Local Ollama (default — no key needed)" },
  { value: "openai_compat", label: "BYOK: OpenAI-compatible (Claude, OpenAI, Mistral, Groq…)" },
];

export default function Settings({ onBack }: Props) {
  const { config, loadConfig, saveConfig } = useAiStore();
  const { accounts, removeAccount } = useAccountStore();

  const [form, setForm] = useState<AiConfig>({
    provider: "ollama",
    base_url: "http://localhost:11434",
    model_triage: "llama3.2:3b",
    model_summary: "llama3.1:8b",
    model_draft: "llama3.1:8b",
    model_extract: "llama3.2:3b",
    model_embed: "nomic-embed-text",
    privacy_mode: false,
    enabled: true,
  });
  const [apiKey, setApiKey] = useState("");
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (config) setForm(config);
  }, [config]);

  const handleSave = async () => {
    await saveConfig(form, apiKey || undefined);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const field = (key: keyof AiConfig) => ({
    value: form[key] as string,
    onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
      setForm((f) => ({ ...f, [key]: e.target.value })),
  });

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <button className={styles.backBtn} onClick={onBack}>← Back</button>
        <h1 className={styles.title}>Settings</h1>
      </div>

      <div className={styles.content}>
        {/* Accounts */}
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>Accounts</h2>
          {accounts.length === 0 && (
            <p className={styles.muted}>No accounts configured.</p>
          )}
          {accounts.map((acc) => (
            <div key={acc.id} className={styles.accountRow}>
              <div>
                <div className={styles.accountEmail}>{acc.email}</div>
                <div className={styles.accountProvider}>{acc.provider}</div>
              </div>
              <button
                className={styles.removeBtn}
                onClick={() => removeAccount(acc.id)}
              >
                Remove
              </button>
            </div>
          ))}
        </section>

        {/* AI */}
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>AI Provider</h2>

          <label className={styles.label}>
            Provider
            <select className={styles.select} value={form.provider} onChange={(e) => setForm((f) => ({ ...f, provider: e.target.value }))}>
              {PROVIDERS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </select>
          </label>

          <label className={styles.label}>
            Base URL
            <input className={styles.input} type="text" {...field("base_url")} />
          </label>

          {form.provider === "openai_compat" && (
            <label className={styles.label}>
              API Key
              <input
                className={styles.input}
                type="password"
                placeholder="sk-... (stored in OS keychain)"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
              />
            </label>
          )}

          <h3 className={styles.subTitle}>Model Assignments</h3>

          <label className={styles.label}>
            Triage / Fast tasks
            <input className={styles.input} type="text" {...field("model_triage")} />
          </label>
          <label className={styles.label}>
            Summarization
            <input className={styles.input} type="text" {...field("model_summary")} />
          </label>
          <label className={styles.label}>
            Reply drafting
            <input className={styles.input} type="text" {...field("model_draft")} />
          </label>
          <label className={styles.label}>
            Action extraction
            <input className={styles.input} type="text" {...field("model_extract")} />
          </label>
          <label className={styles.label}>
            Embeddings
            <input className={styles.input} type="text" {...field("model_embed")} />
          </label>
        </section>

        {/* Privacy */}
        <section className={styles.section}>
          <h2 className={styles.sectionTitle}>Privacy</h2>

          <label className={styles.checkboxLabel}>
            <input
              type="checkbox"
              checked={form.privacy_mode}
              onChange={(e) => setForm((f) => ({ ...f, privacy_mode: e.target.checked }))}
            />
            Privacy mode — strip sender/recipient names before sending to AI API
          </label>

          <label className={styles.checkboxLabel}>
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm((f) => ({ ...f, enabled: e.target.checked }))}
            />
            Enable AI features
          </label>
        </section>

        <button className={styles.saveBtn} onClick={handleSave}>
          {saved ? "Saved ✓" : "Save Settings"}
        </button>
      </div>
    </div>
  );
}
