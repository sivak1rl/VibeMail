import { useEffect, useRef, useState } from "react";
import { useSearchStore } from "../../stores/search";
import { useAccountStore } from "../../stores/accounts";
import styles from "./SearchBar.module.css";

interface Props {
  mailboxId?: string | null;
  onResults: () => void;
  onClear: () => void;
}

export default function SearchBar({ mailboxId = null, onResults, onClear }: Props) {
  const { activeAccountId } = useAccountStore();
  const {
    query,
    searching,
    search,
    searchSemantic,
    reindexing,
    reindexProgress,
    reindexAll,
    clear,
    history,
    loadHistory,
  } = useSearchStore();
  const [localQuery, setLocalQuery] = useState(query);
  const [useSemantic, setUseSemantic] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  useEffect(() => {
    setLocalQuery(query);
  }, [query]);

  // Close history when clicking outside
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setShowHistory(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const runSearch = async (val: string) => {
    if (!activeAccountId) return;
    if (useSemantic) {
      await searchSemantic(val, activeAccountId, mailboxId);
    } else {
      await search(val, activeAccountId, mailboxId);
    }
    setShowHistory(false);
    onResults();
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setLocalQuery(val);

    if (timerRef.current) clearTimeout(timerRef.current);

    if (!val.trim()) {
      clear();
      onClear();
      return;
    }

    timerRef.current = setTimeout(() => {
      void runSearch(val);
    }, 350);
  };

  const handleClear = () => {
    setLocalQuery("");
    clear();
    onClear();
  };

  const handleReindex = () => {
    console.log("SearchBar: handleReindex called for", activeAccountId);
    if (activeAccountId) {
      void reindexAll(activeAccountId);
    }
  };

  const applyFilter = (filter: string) => {
    const newQuery = localQuery.trim() ? `${localQuery} ${filter}` : filter;
    setLocalQuery(newQuery);
    void runSearch(newQuery);
  };

  return (
    <div className={styles.container} ref={containerRef}>
      <div className={styles.wrapper}>
        <span className={styles.icon}>⌕</span>
        <input
          className={styles.input}
          type="search"
          placeholder={useSemantic ? "Semantic search..." : "Search email (try from: or is:unread)"}
          value={localQuery}
          onChange={handleChange}
          onFocus={() => setShowHistory(true)}
        />
        {searching && <span className={styles.spinner}>⟳</span>}
        {localQuery && !searching && (
          <button className={styles.clearBtn} onClick={handleClear}>✕</button>
        )}

        {showHistory && (history.length > 0 || !localQuery) && (
          <div className={styles.historyDropdown}>
            {!localQuery && (
              <div className={styles.filterSection}>
                <div className={styles.historyTitle}>Filters</div>
                <div className={styles.filterGrid}>
                  <button className={styles.filterBtn} onClick={() => applyFilter("is:unread")}>is:unread</button>
                  <button className={styles.filterBtn} onClick={() => applyFilter("has:attachment")}>has:attachment</button>
                  <button className={styles.filterBtn} onClick={() => applyFilter("from:")}>from:...</button>
                  <button className={styles.filterBtn} onClick={() => applyFilter("to:")}>to:...</button>
                </div>
              </div>
            )}
            {history.length > 0 && (
              <>
                <div className={styles.historyTitle}>Recent Searches</div>
                {history.map((h, i) => (
                  <button
                    key={i}
                    className={styles.historyItem}
                    onClick={() => {
                      setLocalQuery(h);
                      void runSearch(h);
                    }}
                  >
                    <span>↺</span> {h}
                  </button>
                ))}
              </>
            )}
          </div>
        )}
      </div>

      <div className={styles.tools}>
        <button
          className={`${styles.toolBtn} ${useSemantic ? styles.toolBtnActive : ""}`}
          onClick={() => {
            setUseSemantic(!useSemantic);
            if (localQuery.trim()) void runSearch(localQuery);
          }}
          title="Search by meaning using AI"
        >
          {useSemantic ? "✨ AI Search On" : "✨ AI Search Off"}
        </button>

        <button
          className={styles.reindexBtn}
          onClick={handleReindex}
          disabled={reindexing}
          title="Generate AI embeddings for all threads"
        >
          {reindexing ? `Reindexing...` : "Reindex AI"}
        </button>
      </div>

      {reindexing && reindexProgress && (
        <div className={styles.progressOverlay}>
          <div className={styles.progressCard}>
            <div className={styles.spinner}>⟳</div>
            <div>{reindexProgress}</div>
          </div>
        </div>
      )}
    </div>
  );
}
