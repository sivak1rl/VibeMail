import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import RoundupWindow from "./components/Roundup/RoundupWindow";
import "./index.css";

const params = new URLSearchParams(window.location.search);
const windowType = params.get("window");

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {windowType === "roundup" ? (
      <RoundupWindow accountId={params.get("accountId") ?? ""} />
    ) : (
      <App />
    )}
  </React.StrictMode>
);
