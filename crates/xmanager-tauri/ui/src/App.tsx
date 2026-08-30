import { useEffect, useState } from "react";
import { layoutFromWidth, store, useAppState, useDrawerOpen } from "./hooks";
import { applyTheme, loadThemeMode, saveThemeMode, type ThemeMode } from "./theme";
import { resolveShortcut } from "./shortcuts";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { LibraryPage } from "./components/library/LibraryPage";
import { InsightsPage } from "./components/insights/InsightsPage";
import { CleanupPage } from "./components/cleanup/CleanupPage";
import { Btn } from "./components/common";

const INSPECTOR_DEFAULT = { wide: 380, medium: 300, narrow: 280 } as const;
const INSPECTOR_MIN = 220;
const INSPECTOR_MAX = 640;

function ErrorBanner({ message }: { message: string }) {
  return (
    <div className="error-banner">
      <span className="error-text">{message}</span>
      <Btn variant="ghost" onClick={() => void store.dispatch({ type: "clear_error" })}>
        关闭
      </Btn>
    </div>
  );
}

export default function App() {
  const { snapshot, pending } = useAppState();
  const drawerOpen = useDrawerOpen();
  const [layout, setLayout] = useState(() => layoutFromWidth(window.innerWidth));
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => loadThemeMode());
  const [inspectorWidth, setInspectorWidth] = useState<number>(INSPECTOR_DEFAULT.wide);

  useEffect(() => {
    const onResize = () => setLayout(layoutFromWidth(window.innerWidth));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  useEffect(() => {
    applyTheme(themeMode);
  }, [themeMode]);

  useEffect(() => {
    void store.initialize();
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!snapshot) return;
      // Modifier chords belong to the OS/webview; never hijack them.
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const resolution = resolveShortcut(event.key, snapshot);
      if (resolution === null) return;
      event.preventDefault();
      switch (resolution.kind) {
        case "action":
          void store.dispatch(resolution.action);
          break;
        case "open_drawer":
          void store.dispatch({ type: "set_route", route: "library" });
          store.openDrawer();
          break;
        case "close_drawer":
          if (drawerOpen) {
            store.closeDrawer();
          } else if (snapshot.focused_tweet_id !== null) {
            void store.dispatch({ type: "unfocus_tweet" });
          }
          break;
        case "unfocus":
          void store.dispatch({ type: "unfocus_tweet" });
          break;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [snapshot, drawerOpen]);

  if (!snapshot) {
    return (
      <div className="app-root">
        <div className="empty-state" style={{ flex: 1 }}>
          <div className="empty-title">正在启动…</div>
        </div>
      </div>
    );
  }

  const changeTheme = (mode: ThemeMode) => {
    setThemeMode(mode);
    saveThemeMode(mode);
  };

  return (
    <div className={`app-root layout-${layout}`}>
      {snapshot.error_msg !== null ? <ErrorBanner message={snapshot.error_msg} /> : null}
      <div className="app-body">
        <Sidebar snapshot={snapshot} compact={layout !== "wide"} themeMode={themeMode} onThemeChange={changeTheme} />
        <main className="app-page">
          <div className={`app-page${snapshot.route === "library" ? "" : " hidden"}`}>
            <LibraryPage
              snapshot={snapshot}
              layout={layout}
              drawerOpen={drawerOpen}
              inspectorWidth={inspectorWidth}
              onInspectorResize={(delta) =>
                setInspectorWidth((width) =>
                  Math.min(INSPECTOR_MAX, Math.max(INSPECTOR_MIN, width - delta))
                )
              }
              onInspectorReset={() => setInspectorWidth(INSPECTOR_DEFAULT[layout])}
            />
          </div>
          <div className={`app-page${snapshot.route === "insights" ? "" : " hidden"}`}>
            <InsightsPage snapshot={snapshot} />
          </div>
          <div className={`app-page${snapshot.route === "cleanup" ? "" : " hidden"}`}>
            <CleanupPage snapshot={snapshot} />
          </div>
        </main>
      </div>
      <StatusBar snapshot={snapshot} pending={pending} />
    </div>
  );
}
