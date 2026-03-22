import { formatDistanceToNow } from "date-fns";
import { useDraftStore, type DraftData } from "../../stores/drafts";
import styles from "./DraftList.module.css";

interface Props {
  drafts: DraftData[];
  selectedId: string | null;
  onOpen: (draft: DraftData) => void;
  onSelect: (id: string) => void;
}

function formatDate(ts: number) {
  try {
    return formatDistanceToNow(new Date(ts * 1000), { addSuffix: true });
  } catch {
    return "";
  }
}

function modeLabel(mode: string) {
  switch (mode) {
    case "reply": return "Reply";
    case "replyAll": return "Reply All";
    case "forward": return "Forward";
    default: return null;
  }
}

export default function DraftList({ drafts, selectedId, onOpen, onSelect }: Props) {
  const { deleteDraft, deleteDraftFromImap } = useDraftStore();

  const handleDelete = async (e: React.MouseEvent, draft: DraftData) => {
    e.stopPropagation();
    await deleteDraft(draft.id);
    void deleteDraftFromImap(draft.id);
  };

  if (drafts.length === 0) return null;

  return (
    <div className={styles.wrapper}>
      <div className={styles.sectionHeader}>
        <span>Local Drafts ({drafts.length})</span>
      </div>
      {drafts.map((draft) => {
        const mode = modeLabel(draft.mode);
        return (
          <div
            key={draft.id}
            className={`${styles.item} ${draft.id === selectedId ? styles.selected : ""}`}
            onClick={() => onSelect(draft.id)}
            onDoubleClick={() => onOpen(draft)}
          >
            <div className={styles.itemTop}>
              <span className={styles.recipients}>
                {mode && <span className={styles.mode}>{mode}</span>}
                {draft.to_addrs || "(no recipients)"}
              </span>
              <div className={styles.meta}>
                <span className={styles.date}>{formatDate(draft.updated_at)}</span>
                <button
                  className={styles.deleteBtn}
                  onClick={(e) => void handleDelete(e, draft)}
                  title="Delete draft"
                >
                  ✕
                </button>
              </div>
            </div>
            <div className={styles.subject}>
              {draft.subject || "(no subject)"}
            </div>
            <div className={styles.snippet}>
              {draft.body_text.slice(0, 120) || "(empty)"}
            </div>
          </div>
        );
      })}
    </div>
  );
}
