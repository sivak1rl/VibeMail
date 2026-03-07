import { useCallback, useEffect, useRef, useState } from "react";
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
  onRefresh?: () => Promise<void>;
  hasMore?: boolean;
  query?: string;
}

const PULL_THRESHOLD = 80;

function Highlight({ text, query }: { text: string; query?: string }) {
  if (!query || !query.trim() || !text) return <>{text}</>;

  // Split query into terms, ignoring filters like from:
  const terms = query
    .split(/\s+/)
    .filter((t) => !t.includes(":"))
    .filter((t) => t.length > 1);

  if (terms.length === 0) return <>{text}</>;

  const regex = new RegExp(`(${terms.join("|")})`, "gi");
  const parts = text.split(regex);

  return (
    <>
      {parts.map((part, i) =>
        regex.test(part) ? (
          <mark key={i} className={styles.mark}>{part}</mark>
        ) : (
          part
        ),
      )}
    </>
  );
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
  onRefresh,
  hasMore,
  query,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const [pullDistance, setPullDistance] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const startY = useRef(0);
  const selectedSet = new Set(selectedThreadIds);

  const handleScroll = useCallback(() => {
    if (!onLoadMore || !hasMore || loading) return;
    const el = containerRef.current;
    if (!el) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 200) {
      onLoadMore();
    }
  }, [onLoadMore, hasMore, loading]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    el.addEventListener("scroll", handleScroll);
    return () => el.removeEventListener("scroll", handleScroll);
  }, [handleScroll]);

  // Pull to refresh logic
  useEffect(() => {
    const el = containerRef.current;
    if (!el || !onRefresh) return;

    const handleTouchStart = (e: TouchEvent) => {
      if (el.scrollTop === 0) {
        startY.current = e.touches[0].pageY;
      } else {
        startY.current = 0;
      }
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (startY.current === 0 || refreshing) return;
      const currentY = e.touches[0].pageY;
      const diff = currentY - startY.current;
      if (diff > 0) {
        setPullDistance(Math.min(diff * 0.5, PULL_THRESHOLD + 20));
        if (diff > 10) e.preventDefault();
      }
    };

    const handleTouchEnd = async () => {
      if (pullDistance >= PULL_THRESHOLD) {
        setRefreshing(true);
        setPullDistance(PULL_THRESHOLD);
        try {
          await onRefresh();
        } finally {
          setRefreshing(false);
          setPullDistance(0);
        }
      } else {
        setPullDistance(0);
      }
      startY.current = 0;
    };

    el.addEventListener("touchstart", handleTouchStart);
    el.addEventListener("touchmove", handleTouchMove, { passive: false });
    el.addEventListener("touchend", handleTouchEnd);
    return () => {
      el.removeEventListener("touchstart", handleTouchStart);
      el.removeEventListener("touchmove", handleTouchMove);
      el.removeEventListener("touchend", handleTouchEnd);
    };
  }, [onRefresh, pullDistance, refreshing]);

  if (loading && threads.length === 0) {
    return (
      <div className={styles.list}>
        {Array.from({ length: 12 }).map((_, i) => (
          <div key={i} className={styles.itemSkeleton}>
            <div className={styles.skeletonLine} style={{ width: "40%", height: "14px" }} />
            <div className={styles.skeletonLine} style={{ width: "80%", height: "12px", marginTop: "8px" }} />
            <div className={styles.skeletonLine} style={{ width: "20%", height: "10px", marginTop: "8px" }} />
          </div>
        ))}
      </div>
    );
  }

  if (threads.length === 0) {
    return (
      <div className={styles.empty}>
        <div className={styles.emptyIcon}>{query ? "🔍" : "📥"}</div>
        <div className={styles.emptyTitle}>
          {query ? "No results found" : "Your inbox is empty"}
        </div>
        <div className={styles.emptyText}>
          {query 
            ? `We couldn't find anything matching "${query}"`
            : "Enjoy the peace and quiet!"}
        </div>
        {onRefresh && (
          <button className={styles.refreshBtn} onClick={() => onRefresh()}>
            Refresh
          </button>
        )}
      </div>
    );
  }

  return (
    <div className={styles.list} ref={containerRef}>
      {pullDistance > 0 && (
        <div 
          className={styles.pullIndicator}
          style={{ height: `${pullDistance}px`, opacity: pullDistance / PULL_THRESHOLD }}
        >
          {refreshing ? "↻" : pullDistance >= PULL_THRESHOLD ? "↑" : "↓"}
        </div>
      )}
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
                <span className={styles.sender}>
                  <Highlight text={senderDisplay} query={query} />
                </span>
              </div>
              <span className={styles.date}>{formatDate(thread.last_date)}</span>
            </div>
            <div className={styles.itemMid}>
              <div style={{ display: "flex", alignItems: "center", minWidth: 0 }}>
                {thread.is_flagged && <span className={styles.star}>★</span>}
                {thread.has_attachments && <span className={styles.paperclip}>📎</span>}
                <span className={styles.subject}>
                  <Highlight text={thread.subject ?? "(no subject)"} query={query} />
                </span>
              </div>
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
