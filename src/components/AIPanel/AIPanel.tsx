import { useState } from "react";
import type { Message, Thread } from "../../stores/threads";
import { useAiStore } from "../../stores/ai";
import styles from "./AIPanel.module.css";

interface Props {
  thread: Thread;
  messages?: Message[];
}

export default function AIPanel({ thread }: Props) {
  const {
    config,
    summaryByThread,
    summaryStreaming,
    actionsByThread,
    summarizeThread,
    extractActions,
    triageThread,
  } = useAiStore();

  const [collapsed, setCollapsed] = useState(false);

  const summary = summaryByThread[thread.id];
  const isStreamingSummary = summaryStreaming[thread.id];
  const actions = actionsByThread[thread.id];

  if (!config?.enabled) {
    return (
      <div className={styles.panel}>
        <div className={styles.header}>
          <span className={styles.title}>AI</span>
        </div>
        <div className={styles.setupPrompt}>
          <p>Set up an AI provider in Settings to enable smart features.</p>
        </div>
      </div>
    );
  }

  if (collapsed) {
    return (
      <div className={`${styles.panel} ${styles.panelCollapsed}`}>
        <button className={styles.expandBtn} onClick={() => setCollapsed(false)} title="Expand AI Panel">
          ✦
        </button>
      </div>
    );
  }

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <span className={styles.title}>✦ AI</span>
        <button className={styles.collapseBtn} onClick={() => setCollapsed(true)} title="Collapse">
          ›
        </button>
      </div>

      <div className={styles.section}>
        <div className={styles.sectionHeader}>
          <span className={styles.sectionTitle}>Summary</span>
          <button
            className={styles.actionBtn}
            onClick={() => summarizeThread(thread.id)}
            disabled={isStreamingSummary}
          >
            {isStreamingSummary ? "..." : "Generate"}
          </button>
        </div>
        {summary && (
          <p className={styles.summary}>
            {summary}
            {isStreamingSummary && <span className={styles.cursor}>▌</span>}
          </p>
        )}
      </div>

      <div className={styles.section}>
        <div className={styles.sectionHeader}>
          <span className={styles.sectionTitle}>Actions</span>
          <button
            className={styles.actionBtn}
            onClick={() => extractActions(thread.id)}
          >
            Extract
          </button>
        </div>
        {actions && actions.length === 0 && (
          <p className={styles.muted}>No actions found</p>
        )}
        {actions && actions.length > 0 && (
          <ul className={styles.actionList}>
            {actions.map((a, i) => (
              <li key={i} className={styles.actionItem}>
                <span className={styles.actionKind}>
                  {a.kind === "date" ? "📅" : a.kind === "followup" ? "🔔" : "☐"}
                </span>
                <span className={styles.actionText}>{a.text}</span>
                {a.date && <span className={styles.actionDate}>{a.date}</span>}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className={styles.section}>
        <div className={styles.sectionHeader}>
          <span className={styles.sectionTitle}>Priority</span>
          <button
            className={styles.actionBtn}
            onClick={() => triageThread(thread.id)}
          >
            Score
          </button>
        </div>
        {thread.triage_score !== null && (
          <div className={styles.scoreBar}>
            <div
              className={styles.scoreFill}
              style={{
                width: `${Math.round((thread.triage_score ?? 0) * 100)}%`,
                background:
                  (thread.triage_score ?? 0) >= 0.75
                    ? "var(--color-high-priority)"
                    : (thread.triage_score ?? 0) >= 0.4
                    ? "var(--color-warning)"
                    : "var(--color-border)",
              }}
            />
            <span className={styles.scoreLabel}>
              {Math.round((thread.triage_score ?? 0) * 100)}%
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
