import { useEffect, useState } from "react";

export function App() {
  const [refreshes, setRefreshes] = useState(0);

  useEffect(() => {
    const off = muxy.events.subscribe("command.refresh-hello", () => setRefreshes((n) => n + 1));
    return () => off?.();
  }, []);

  return (
    <div className="panel">
      <div className="title">Hello from Muxy</div>
      <p className="caption">A starter panel that follows the theme and the sizing scale.</p>
      <div className="card">
        <span>Refreshes</span>
        <span className="count">{refreshes}</span>
      </div>
      <button className="button" onClick={() => setRefreshes((n) => n + 1)}>
        Refresh
      </button>
    </div>
  );
}
