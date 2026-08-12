import { Maximize2, Minus, X } from "lucide-react";
const windowControl = async (action: "minimize" | "toggleMaximize" | "close") => {
  // The Vite preview runs in a normal browser, where Tauri's window API is absent.
  if (!("__TAURI_INTERNALS__" in window)) return;

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow()[action]();
};

export function WindowControls() {
  return (
    <div className="window-controls" data-tauri-drag-region="false" aria-label="Window controls">
      <button type="button" onClick={() => void windowControl("minimize")} aria-label="Minimize Coldem">
        <Minus size={15} strokeWidth={2.4} />
      </button>
      <button type="button" onClick={() => void windowControl("toggleMaximize")} aria-label="Maximize Coldem">
        <Maximize2 size={13} strokeWidth={2.4} />
      </button>
      <button className="window-controls__close" type="button" onClick={() => void windowControl("close")} aria-label="Close Coldem">
        <X size={16} strokeWidth={2.5} />
      </button>
    </div>
  );
}
