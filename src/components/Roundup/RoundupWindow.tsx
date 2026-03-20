import { useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { useAiStore, type ThreadHighlight } from "../../stores/ai";
import styles from "./RoundupWindow.module.css";

const WINDOWS = [
  { label: "Today", days: 1 },
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
] as const;

interface Props {
  accountId: string;
}

export default function RoundupWindow({ accountId }: Props) {
  const [selectedDays, setSelectedDays] = useState<number>(1);
  const [expandedThread, setExpandedThread] = useState<string | null>(null);
  const {
    roundup,
    roundupStreaming,
    roundupNarrative,
    generateRoundup,
    summarizeThread,
    summaryByThread,
    summaryStreaming,
    draftByThread,
    draftStreaming,
    categorizeThreads,
  } = useAiStore();

  useEffect(() => {
    if (accountId) {
      void generateRoundup(accountId, selectedDays);
    }
  }, [accountId, selectedDays, generateRoundup]);

  const triageDotClass = (score: number) => {
    if (score >= 0.7) return styles.dotHigh;
    if (score >= 0.4) return styles.dotMed;
    return styles.dotLow;
  };

  const handleOpen = (threadId: string) => {
    void emit("roundup:open-thread", { threadId });
  };

  const handleReply = (threadId: string) => {
    void emit("roundup:reply-thread", { threadId });
  };

  const handleSummarize = (threadId: string) => {
    setExpandedThread(threadId);
    void summarizeThread(threadId);
  };

  const handleLabel = async (threadId: string) => {
    const results = await categorizeThreads([threadId], undefined, true);
    if (results.length > 0) {
      // Update the highlight's labels in-place via the roundup store
      useAiStore.setState((s) => {
        if (!s.roundup) return {};
        return {
          roundup: {
            ...s.roundup,
            highlights: s.roundup.highlights.map((h) =>
              h.thread_id === threadId
                ? { ...h, labels: [...new Set([...h.labels, results[0].label])] }
                : h
            ),
          },
        };
      });
    }
  };

  const toggleExpanded = (threadId: string) => {
    setExpandedThread((prev) => (prev === threadId ? null : threadId));
  };

  return (
    <div className={styles.container}>
      <div className={styles.header} data-tauri-drag-region>
        <div className={styles.headerLeft}>
          <span className={styles.title}>Inbox Roundup</span>
          <div className={styles.tabs}>
            {WINDOWS.map((w) => (
              <button
                key={w.days}
                className={`${styles.tab} ${selectedDays === w.days ? styles.tabActive : ""}`}
                onClick={() => setSelectedDays(w.days)}
                disabled={roundupStreaming}
              >
                {w.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {roundup && (
        <div className={styles.stats}>
          <span>{roundup.total_threads} threads</span>
          <span className={styles.statDot}>·</span>
          <span>{roundup.unread_count} unread</span>
          <span className={styles.statDot}>·</span>
          <span>{roundup.action_item_count} with actions</span>
        </div>
      )}

      <div className={styles.narrative}>
        {roundupStreaming && !roundupNarrative && (
          <span className={styles.generating}>Generating roundup…</span>
        )}
        {roundupNarrative && (
          <p className={styles.narrativeText}>
            {roundupNarrative}
            {roundupStreaming && <span className={styles.cursor}>▋</span>}
          </p>
        )}
      </div>

      {roundup && roundup.highlights.length > 0 && (
        <div className={styles.highlights}>
          <div className={styles.highlightsLabel}>Top threads</div>
          {roundup.highlights.map((h) => (
            <HighlightItem
              key={h.thread_id}
              highlight={h}
              expanded={expandedThread === h.thread_id}
              summary={summaryByThread[h.thread_id]}
              summaryLoading={!!summaryStreaming[h.thread_id]}
              draft={draftByThread[h.thread_id]}
              draftLoading={!!draftStreaming[h.thread_id]}
              triageDotClass={triageDotClass}
              onToggle={() => toggleExpanded(h.thread_id)}
              onOpen={() => handleOpen(h.thread_id)}
              onReply={() => handleReply(h.thread_id)}
              onSummarize={() => handleSummarize(h.thread_id)}
              onLabel={() => void handleLabel(h.thread_id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface HighlightItemProps {
  highlight: ThreadHighlight;
  expanded: boolean;
  summary?: string;
  summaryLoading: boolean;
  draft?: string;
  draftLoading: boolean;
  triageDotClass: (score: number) => string;
  onToggle: () => void;
  onOpen: () => void;
  onReply: () => void;
  onSummarize: () => void;
  onLabel: () => void;
}

function HighlightItem({
  highlight: h,
  expanded,
  summary,
  summaryLoading,
  draft,
  draftLoading,
  triageDotClass,
  onToggle,
  onOpen,
  onReply,
  onSummarize,
  onLabel,
}: HighlightItemProps) {
  return (
    <div className={`${styles.highlightItem} ${expanded ? styles.highlightExpanded : ""}`}>
      <span
        className={`${styles.triageDot} ${triageDotClass(h.triage_score)}`}
        onClick={onToggle}
      />
      <div className={styles.highlightContent}>
        <div className={styles.highlightMeta} onClick={onToggle}>
          <span className={styles.highlightSubject}>{h.subject}</span>
          {h.unread && <span className={styles.unreadBadge}>•</span>}
          {h.labels.map((l) => (
            <span key={l} className={styles.label}>
              {l}
            </span>
          ))}
        </div>
        <div className={styles.highlightFrom}>{h.last_from}</div>
        {h.summary && !expanded && (
          <div className={styles.highlightSummary}>{h.summary}</div>
        )}

        {expanded && (
          <div className={styles.expandedPanel}>
            <div className={styles.actions}>
              <button className={styles.actionBtn} onClick={onOpen} title="Open in main window">
                Open
              </button>
              <button className={styles.actionBtn} onClick={onReply} title="Draft a reply">
                Reply
              </button>
              <button
                className={styles.actionBtn}
                onClick={onSummarize}
                disabled={summaryLoading}
                title="AI summarize"
              >
                {summaryLoading ? "Summarizing…" : "Summarize"}
              </button>
              <button className={styles.actionBtn} onClick={onLabel} title="Auto-label">
                Label
              </button>
            </div>

            {(summary || summaryLoading) && (
              <div className={styles.aiResult}>
                <div className={styles.aiResultLabel}>Summary</div>
                <p className={styles.aiResultText}>
                  {summary}
                  {summaryLoading && <span className={styles.cursor}>▋</span>}
                </p>
              </div>
            )}

            {(draft || draftLoading) && (
              <div className={styles.aiResult}>
                <div className={styles.aiResultLabel}>Draft reply</div>
                <p className={styles.aiResultText}>
                  {draft}
                  {draftLoading && <span className={styles.cursor}>▋</span>}
                </p>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
