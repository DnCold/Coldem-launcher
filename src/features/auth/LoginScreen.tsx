import {
  ArrowRight,
  Gamepad2,
  LoaderCircle,
  PackageCheck,
  Sparkles
} from "lucide-react";
import { BrandMark } from "../../components/BrandMark";
import { PetSticker } from "../../components/PetSticker";
import { WindowControls } from "../../components/WindowControls";
import runnerWelcome from "../../assets/runner-welcome.png";

interface LoginScreenProps {
  onLogin: () => Promise<void>;
  onCancel: () => Promise<void>;
  onOpenExternal: (target: string) => Promise<void>;
  authenticating: boolean;
  error: string | null;
}

export function LoginScreen({
  onLogin,
  onCancel,
  onOpenExternal,
  authenticating,
  error
}: LoginScreenProps) {
  return (
    <main className="login-shell">
      <div className="window-bar window-bar--login" data-tauri-drag-region>
        <span className="window-bar__label">COLD<span>EM</span> // DANCOLD GAMES</span>
        <WindowControls />
      </div>
      <div className="login-grunge" aria-hidden="true">
        <span>&#10022;</span><i>DNCLD // PRIVATE ARCADE</i><b>///</b>
      </div>
      <section className="login-art" aria-hidden="true">
        <div className="login-art__brand">
          <BrandMark />
          <div className="brand-motto">
            <span>Colder than all.</span><strong>Always.</strong>
          </div>
        </div>
        <div className="login-art__poster">
          <span className="feature-tag"><Sparkles size={12} /> Friends edition</span>
          <h2>Stay cold.<br />Play hard.</h2>
          <p>Small strange worlds, delivered directly by DanCold.</p>
          <div className="login-art__scribble">PLAY // BUILD // REPEAT</div>
        </div>
        <figure className="login-runner">
          <img src={runnerWelcome} alt="" />
          <figcaption>THE RUNNER // YOUR GAME COURIER</figcaption>
        </figure>
        <div className="login-pets">
          <PetSticker kind="yin" variant={2} decorative />
          <PetSticker kind="yang" variant={3} decorative />
          <PetSticker kind="deadpool" variant={4} decorative />
          <PetSticker kind="wolverine" variant={2} decorative />
        </div>
        <div className="login-art__copy">
          <Gamepad2 size={18} />
          <p>Yin, Yang, Deadpool and Wolverine are guarding the library.</p>
        </div>
      </section>

      <section className="login-panel">
        <div className="login-panel__header">
          <BrandMark />
          <span>DANCOLD CREATOR BUILD</span>
        </div>
        <div className="login-panel__body">
          <p className="eyebrow">WELCOME TO THE COLD SIDE</p>
          <h1>Your games.<br />One cold place.</h1>
          <p className="login-panel__intro">
            Enter the Coldem library and install DanCold games directly. No
            account, API key, or password is required.
          </p>

          <div className="oauth-login">
            <button
              type="button"
              className="primary-button primary-button--wide"
              disabled={authenticating}
              onClick={() => void onLogin()}
            >
              <span>
                {authenticating ? "Opening the library..." : "Enter Coldem"}
              </span>
              {authenticating ? (
                <LoaderCircle className="spin" size={18} />
              ) : (
                <ArrowRight size={18} />
              )}
            </button>

            {authenticating && (
              <div className="oauth-login__waiting" role="status">
                <p>Preparing the public game catalog...</p>
                <button type="button" className="text-button" onClick={() => void onCancel()}>
                  Cancel
                </button>
              </div>
            )}

            {error && <p className="form-error" role="alert">{error}</p>}
          </div>

          <button
            type="button"
            className="text-button"
            onClick={() => void onOpenExternal("https://github.com/features/releases")}
          >
            How are games delivered? <ArrowRight size={14} />
          </button>

          <div className="security-note">
            <PackageCheck size={16} />
            <p>
              Downloads come from public GitHub Releases. Coldem checks every
              file before butler installs or patches it on your computer.
            </p>
          </div>
        </div>
        <p className="login-panel__footer">GITHUB RELEASES + WHARF PATCHES <span>&#10022; STAY COLD</span></p>
      </section>
    </main>
  );
}
