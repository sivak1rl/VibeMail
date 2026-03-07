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
  const { query, searching, search, searchSemantic, reindexing, reindexProgress, reindexAll, clear } = useSearchStore();
  const [localQuery, setLocalQuery] = useState(query);
  const [useSemantic, setUseSemantic] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setLocalQuery(query);
  }, [query]);

  const runSearch = async (val: string) => {
    if (!activeAccountId) return;
    if (useSemantic) {
      await searchSemantic(val, activeAccountId, mailboxId);
    } else {
      await search(val, activeAccountId, mailboxId);
    }
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

  return (
    <div className={styles.container}>
      <div className={styles.wrapper}>
        <span className={styles.icon}>⌕</span>
        <input
          className={styles.input}
          type="search"
          placeholder={useSemantic ? "Semantic search..." : "Search email..."}
          value={localQuery}
          onChange={handleChange}
        />
        {searching && <span className={styles.spinner}>⟳</span>}
        {localQuery && !searching && (
          <button className={styles.clearBtn} onClick={handleClear}>✕</button>
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
