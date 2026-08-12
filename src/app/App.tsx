import { AlertTriangle, LoaderCircle } from "lucide-react";
import { BrandMark } from "../components/BrandMark";
import { LoginScreen } from "../features/auth/LoginScreen";
import { LibraryPage } from "../features/library/LibraryPage";
import { PromptDialog } from "../features/library/PromptDialog";
import { isDemoMode } from "../lib/launcherClient";
import { useLauncher } from "../hooks/useLauncher";
import { PetSticker } from "../components/PetSticker";

export function App() {
  const launcher = useLauncher();

  if (launcher.status === "booting") {
    return (
      <main className="boot-screen">
        <div className="screen-graffiti" aria-hidden="true">WAKE // PLAY // REPEAT</div>
        <BrandMark />
        <PetSticker kind="yin" variant={2} decorative />
        <LoaderCircle className="spin" size={22} />
        <p>Waking the library</p>
      </main>
    );
  }

  if (launcher.status === "error") {
    return (
      <main className="error-screen">
        <div className="screen-graffiti" aria-hidden="true">SYSTEM // TOO WARM</div>
        <BrandMark />
        <div className="error-card">
          <PetSticker kind="wolverine" variant={4} decorative />
          <AlertTriangle size={24} />
          <div>
            <h1>Butler isn’t ready</h1>
            <p>{launcher.error}</p>
            <code>pnpm sidecar:fetch</code>
          </div>
        </div>
      </main>
    );
  }

  if (launcher.status === "login" || !launcher.profile) {
    return (
      <LoginScreen
        onLogin={launcher.login}
        onCancel={launcher.cancelLogin}
        onOpenExternal={launcher.openExternal}
        authenticating={launcher.isAuthenticating}
        error={launcher.authError}
      />
    );
  }

  return (
    <>
      {isDemoMode && <div className="demo-badge">Browser preview</div>}
      <LibraryPage launcher={launcher} />
      {launcher.prompt && (
        <PromptDialog
          prompt={launcher.prompt}
          onRespond={launcher.respondToPrompt}
        />
      )}
    </>
  );
}
