import { useEffect, useState } from "react";
import { useAccountStore } from "./stores/accounts";
import Inbox from "./pages/Inbox";
import Settings from "./pages/Settings";
import AccountSetup from "./pages/AccountSetup";
import styles from "./App.module.css";

type Page = "inbox" | "settings" | "setup";

export default function App() {
  const { accounts, fetchAccounts } = useAccountStore();
  const [page, setPage] = useState<Page>("inbox");
  const [initialLoad, setInitialLoad] = useState(true);

  useEffect(() => {
    fetchAccounts().then(() => setInitialLoad(false));
  }, [fetchAccounts]);

  useEffect(() => {
    if (!initialLoad && accounts.length === 0) {
      setPage("setup");
    }
  }, [accounts, initialLoad]);

  if (initialLoad) {
    return <div className={styles.app} />;
  }

  return (
    <div className={styles.app}>
      {page === "setup" && <AccountSetup onDone={() => setPage("inbox")} />}
      {page === "inbox" && (
        <Inbox onSettings={() => setPage("settings")} />
      )}
      {page === "settings" && (
        <Settings onBack={() => setPage("inbox")} />
      )}
    </div>
  );
}
