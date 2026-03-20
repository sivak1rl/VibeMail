import { useEffect, useState } from "react";
import { useAiStore } from "../../stores/ai";
import styles from "./Roundup.module.css";

const WINDOWS = [
  { label: "Today", days: 1 },
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
] as const;

interface Props {
  accountId: string;
  onClose: () => void;
}

export default function Roundup({ accountId, onClose }: Props) {
  const [selectedDays, setSelectedDays] = useState<number>(1);
  const { roundup, roundupStreaming, roundupNarrative, generateRoundup } = useAiStore();

  useEffect(() => {
    void generateRoundup(accountId, selectedDays);
  }, [accountId, selectedDays, generateRoundup]);

  const triageDotClass = (score: number) => {
    if (score >= 0.7) return styles.dotHigh;
    if (score >= 0.4) return styles.dotMed;
    return styles.dotLow;
  };

  return (
    <div className={styles.backdrop} onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
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
          <button className={styles.closeBtn} onClick={onClose} title="Close">
            ✕
          </button>
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
              <div key={h.thread_id} className={styles.highlightItem}>
                <span className={`${styles.triageDot} ${triageDotClass(h.triage_score)}`} />
                <div className={styles.highlightContent}>
                  <div className={styles.highlightMeta}>
                    <span className={styles.highlightSubject}>{h.subject}</span>
                    {h.unread && <span className={styles.unreadBadge}>•</span>}
                    {h.labels.map((l) => (
                      <span key={l} className={styles.label}>
                        {l}
                      </span>
                    ))}
                  </div>
                  <div className={styles.highlightFrom}>{h.last_from}</div>
                  {h.summary && (
                    <div className={styles.highlightSummary}>{h.summary}</div>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
