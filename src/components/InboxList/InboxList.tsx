import { useCallback, useEffect, useRef } from "react";
import { formatDistanceToNow } from "date-fns";
import type { Thread } from "../../stores/threads";
import styles from "./InboxList.module.css";

interface Props {
  threads: Thread[];
  selectedId: string | null;
  selectedThreadIds: string[];
  onSelect: (id: string) => void;
  onToggleSelect: (id: string, selected: boolean, withShift: boolean) => void;
  loading: boolean;
  onLoadMore?: () => void;
  hasMore?: boolean;
}

function TriageDot({ score }: { score: number | null }) {
  if (score === null) return null;
  const cls =
    score >= 0.75
      ? styles.dotHigh
      : score >= 0.4
      ? styles.dotMedium
      : styles.dotLow;
  return <span className={`${styles.dot} ${cls}`} title={`Priority: ${Math.round((score ?? 0) * 100)}%`} />;
}

function formatDate(dateStr: string | null) {
  if (!dateStr) return "";
  try {
    return formatDistanceToNow(new Date(dateStr), { addSuffix: true });
  } catch {
    return "";
  }
}

function categoryLabel(labels: string[]): string | null {
  for (const label of labels) {
    if (label === "newsletter") return "Newsletter";
    if (label === "receipt") return "Receipt";
    if (label === "social") return "Social";
    if (label === "updates") return "Updates";
    if (label.trim().length > 0) {
      return label
        .replace(/[_-]+/g, " ")
        .replace(/\b\w/g, (ch) => ch.toUpperCase());
    }
  }
  return null;
}

export default function InboxList({
  threads,
  selectedId,
  selectedThreadIds,
  onSelect,
  onToggleSelect,
  loading,
  onLoadMore,
  hasMore,
}: Props) {
  const sentinelRef = useRef<HTMLDivElement>(null);
  const selectedSet = new Set(selectedThreadIds);

  const handleScroll = useCallback(() => {
    if (!onLoadMore || !hasMore) return;
    const el = sentinelRef.current?.parentElement;
    if (!el) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 200) {
      onLoadMore();
    }
  }, [onLoadMore, hasMore]);

  useEffect(() => {
    const el = sentinelRef.current?.parentElement;
    if (!el) return;
    el.addEventListener("scroll", handleScroll);
    return () => el.removeEventListener("scroll", handleScroll);
  }, [handleScroll]);

  if (loading && threads.length === 0) {
    return (
      <div className={styles.list}>
        {Array.from({ length: 8 }).map((_, i) => (
          <div key={i} className={styles.skeleton} />
        ))}
      </div>
    );
  }

  if (threads.length === 0) {
    return (
      <div className={styles.empty}>
        <p>No messages</p>
      </div>
    );
  }

  return (
    <div className={styles.list}>
      {threads.map((thread) => {
        const isUnread = thread.unread_count > 0;
        const isSelected = thread.id === selectedId;
        const isChecked = selectedSet.has(thread.id);
        const senderDisplay =
          thread.last_from ?? thread.participants[0]?.email ?? "Unknown";
        const category = categoryLabel(thread.labels);

        return (
          <div
            key={thread.id}
            className={`${styles.item} ${isSelected ? styles.selected : ""} ${
              isUnread ? styles.unread : ""
            }`}
            onClick={() => onSelect(thread.id)}
          >
            <div className={styles.itemTop}>
              <div className={styles.itemLead}>
                <input
                  type="checkbox"
                  checked={isChecked}
                  className={styles.selectCheckbox}
                  onClick={(event) => {
                    event.stopPropagation();
                    event.preventDefault();
                    onToggleSelect(thread.id, !isChecked, event.shiftKey);
                  }}
                />
                <span className={styles.sender}>{senderDisplay}</span>
              </div>
              <span className={styles.date}>{formatDate(thread.last_date)}</span>
            </div>
            <div className={styles.itemMid}>
              <span className={styles.subject}>
                {thread.subject ?? "(no subject)"}
              </span>
              {thread.unread_count > 0 && (
                <span className={styles.unreadBadge}>{thread.unread_count}</span>
              )}
            </div>
            <div className={styles.itemBot}>
              <TriageDot score={thread.triage_score} />
              {category && <span className={styles.category}>{category}</span>}
              {thread.message_count > 1 && (
                <span className={styles.count}>{thread.message_count}</span>
              )}
            </div>
          </div>
        );
      })}
      <div ref={sentinelRef} />
      {loading && threads.length > 0 && (
        <div className={styles.loadingMore}>Loading more…</div>
      )}
    </div>
  );
}
