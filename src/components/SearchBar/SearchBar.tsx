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
  const { query, searching, search, clear } = useSearchStore();
  const [localQuery, setLocalQuery] = useState(query);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setLocalQuery(query);
  }, [query]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setLocalQuery(val);

    if (timerRef.current) clearTimeout(timerRef.current);

    if (!val.trim()) {
      clear();
      onClear();
      return;
    }

    timerRef.current = setTimeout(async () => {
      if (!activeAccountId) return;
      await search(val, activeAccountId, mailboxId);
      onResults();
    }, 350);
  };

  const handleClear = () => {
    setLocalQuery("");
    clear();
    onClear();
  };

  return (
    <div className={styles.wrapper}>
      <span className={styles.icon}>⌕</span>
      <input
        className={styles.input}
        type="search"
        placeholder="Search email..."
        value={localQuery}
        onChange={handleChange}
      />
      {searching && <span className={styles.spinner}>⟳</span>}
      {localQuery && !searching && (
        <button className={styles.clearBtn} onClick={handleClear}>✕</button>
      )}
    </div>
  );
}
