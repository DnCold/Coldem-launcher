import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  Bell,
  Clock3,
  Download,
  Gamepad2,
  HardDrive,
  Home,
  Library,
  LoaderCircle,
  LogOut,
  Music2,
  RefreshCw,
  Search,
  Settings2,
  Sparkles,
  Zap
} from "lucide-react";
import { BrandMark } from "../../components/BrandMark";
import { PetSticker } from "../../components/PetSticker";
import { WindowControls } from "../../components/WindowControls";
import { AppUpdateButton } from "../app-update/AppUpdateButton";
import { SocialPanel } from "../social/SocialPanel";
import { useSocial } from "../../hooks/useSocial";
import type { useLauncher } from "../../hooks/useLauncher";
import type { GameRecord, InstallOptions, Upload } from "../../types/launcher";
import dancoldLogo from "../../assets/dancold-logo.png";
import runnerActive from "../../assets/runner-active.png";
import runnerHead from "../../assets/runner-head.png";
import { GameCard } from "./GameCard";
import { InstallDialog } from "./InstallDialog";
import { applySavedVisualPreferences, SettingsDialog } from "./SettingsDialog";

type LauncherState = ReturnType<typeof useLauncher>;
type LibraryFilter = "home" | "all" | "installed" | "updates";

interface LibraryPageProps {
  launcher: LauncherState;
}

const formatPlaytime = (seconds: number) => {
  if (seconds < 60) return "Just installed";
  if (seconds < 3600) return `${Math.max(1, Math.round(seconds / 60))}m played`;
  return `${Math.round(seconds / 3600)}h played`;
};

const formatLiveSession = (seconds: number) => {
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  if (minutes < 1) return `LIVE // ${String(remainder).padStart(2, "0")}s`;
  return `LIVE // ${minutes}m ${String(remainder).padStart(2, "0")}s`;
};

export function LibraryPage({ launcher }: LibraryPageProps) {
  const social = useSocial();
  const [filter, setFilter] = useState<LibraryFilter>("home");
  const [query, setQuery] = useState("");
  const [installOptions, setInstallOptions] = useState<InstallOptions | null>(null);
  const [installingRecord, setInstallingRecord] = useState<GameRecord | null>(null);
  const [inviteInstallPayload, setInviteInstallPayload] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    applySavedVisualPreferences();
  }, []);

  const caveByGame = useMemo(
    () => new Map(launcher.library?.caves.map((cave) => [cave.game.id, cave])),
    [launcher.library?.caves]
  );
  const updateByGame = useMemo(
    () => new Map(launcher.updates.map((update) => [update.game.id, update])),
    [launcher.updates]
  );

  const games = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return (launcher.library?.records ?? []).filter((game) => {
      if (normalizedQuery && !game.title.toLocaleLowerCase().includes(normalizedQuery)) return false;
      if (filter === "installed") return caveByGame.has(game.id);
      if (filter === "updates") return updateByGame.has(game.id);
      return true;
    });
  }, [caveByGame, filter, launcher.library?.records, query, updateByGame]);

  const pendingInvite = social.snapshot?.pendingJoin;
  const invitePayload = pendingInvite
    ? `coldem:v1:${pendingInvite.gameSlug}:${pendingInvite.code}`
    : null;
  const invitedRecord = useMemo(
    () => pendingInvite
      ? launcher.library?.records.find((record) => record.slug === pendingInvite.gameSlug)
      : undefined,
    [launcher.library?.records, pendingInvite]
  );
  const invitedCave = invitedRecord ? caveByGame.get(invitedRecord.id) : undefined;
  const invitedOperation = invitedRecord ? launcher.operations[invitedRecord.id] : undefined;
  const isInvitedGameRunning = invitedOperation?.kind === "play" && invitedOperation.state === "running";

  const recentCave = useMemo(
    () => [...(launcher.library?.caves ?? [])].sort((a, b) =>
      Date.parse(b.interaction?.lastRunAt ?? b.stats.lastTouchedAt ?? b.stats.installedAt ?? "0") -
      Date.parse(a.interaction?.lastRunAt ?? a.stats.lastTouchedAt ?? a.stats.installedAt ?? "0")
    )[0],
    [launcher.library?.caves]
  );

  const featuredRecord = useMemo(() => {
    const records = launcher.library?.records ?? [];
    return records.find((record) => record.id === recentCave?.game.id) ?? records[0];
  }, [launcher.library?.records, recentCave?.game.id]);
  const featuredCave = featuredRecord ? caveByGame.get(featuredRecord.id) : undefined;
  const featuredUpdate = featuredRecord ? updateByGame.get(featuredRecord.id) : undefined;
  const featuredOperation = featuredRecord ? launcher.operations[featuredRecord.id] : undefined;
  const isFeaturedBusy = featuredOperation && !["finished", "failed"].includes(featuredOperation.state);
  const isFeaturedPlaying = featuredOperation?.kind === "play" && featuredOperation.state === "running";
  const playingOperation = useMemo(
    () => Object.values(launcher.operations).find((operation) => operation.kind === "play" && operation.state === "running"),
    [launcher.operations]
  );
  const playingRecord = playingOperation
    ? launcher.library?.records.find((record) => record.id === playingOperation.gameId)
    : undefined;
  const [sessionNow, setSessionNow] = useState(() => Date.now());

  useEffect(() => {
    if (!playingOperation) return;
    setSessionNow(Date.now());
    const interval = window.setInterval(() => setSessionNow(Date.now()), 1000);
    return () => window.clearInterval(interval);
  }, [playingOperation]);

  const liveSessionSeconds = playingOperation
    ? Math.max(0, Math.floor((sessionNow - playingOperation.startedAt) / 1000))
    : 0;

  const activeOperations = useMemo(
    () => Object.values(launcher.operations).filter((operation) =>
      !["finished", "failed"].includes(operation.state) && operation.kind !== "play"
    ),
    [launcher.operations]
  );

  const openInstall = async (game: GameRecord) => {
    setInstallingRecord(game);
    setActionError(null);
    try {
      setInstallOptions(await launcher.prepareInstall(game.id));
    } catch (reason) {
      setActionError(String(reason));
      setInstallingRecord(null);
    }
  };

  const closeInstall = () => {
    setInstallOptions(null);
    setInstallingRecord(null);
  };

  const performInstall = (upload: Upload) => {
    if (!installOptions) return;
    const game = installOptions.game;
    const pendingPayload = inviteInstallPayload;
    safely((async () => {
      await launcher.install(game, upload, installOptions.installLocationId);
      if (!pendingPayload) return;
      const refreshed = await launcher.refresh();
      const cave = refreshed?.caves.find((candidate) => candidate.game.id === game.id);
      if (!cave) throw new Error("Robot Rock installed, but Coldem could not find its launch record.");
      await launcher.play(cave.id, game.id, pendingPayload);
      setInviteInstallPayload(null);
      await social.dismissJoin();
    })());
  };

  const safely = (work: Promise<void>) => {
    setActionError(null);
    void work.catch((reason) => setActionError(String(reason)));
  };

  const runFeatured = () => {
    if (!featuredRecord || isFeaturedBusy) return;
    if (featuredCave && featuredUpdate) {
      safely(launcher.update(featuredCave.id, featuredRecord.id));
    } else if (featuredCave) {
      safely(launcher.play(featuredCave.id, featuredRecord.id));
    } else {
      void openInstall(featuredRecord);
    }
  };

  const joinInvite = () => {
    if (!invitedRecord || !invitedCave || !invitePayload) return;
    safely((async () => {
      await launcher.play(invitedCave.id, invitedRecord.id, invitePayload);
      await social.dismissJoin();
    })());
  };

  const installAndJoinInvite = () => {
    if (!invitedRecord || !invitePayload) return;
    setInviteInstallPayload(invitePayload);
    void openInstall(invitedRecord);
  };

  const queueInvite = () => {
    if (!invitedRecord || !invitePayload) return;
    safely((async () => {
      await social.queueJoinForRunningGame(invitedRecord.id, invitePayload);
    })());
  };

  const featuredAction = isFeaturedBusy
    ? featuredOperation.kind === "play" ? "Running" : `${Math.round((featuredOperation.progress ?? 0) * 100)}%`
    : featuredUpdate ? "Update" : featuredCave ? "Play" : "Install";
  const installedCount = launcher.library?.caves.length ?? 0;
  const updatesCount = launcher.updates.length;

  return (
    <main className="app-shell">
      <div className="window-bar window-bar--app" data-tauri-drag-region>
        <span className="window-bar__label">COLD<span>EM</span> // DANCOLD GAMES</span>
        <WindowControls />
      </div>
      <div className="grunge-overlay" aria-hidden="true">
        <span className="grunge-overlay__star">★</span>
        <span className="grunge-overlay__cross">× × ×</span>
        <span className="grunge-overlay__slashes">///</span>
        <span className="grunge-overlay__code">DNCLD // COLD DELIVERY</span>
      </div>
      <div className="street-fx" aria-hidden="true">
        <span className="street-fx__orbit" />
        <span className="street-fx__crown">♕</span>
        <span className="street-fx__tag">COLD<br /><b>MODE</b></span>
        <span className="street-fx__spark">✦</span>
        <span className="street-fx__arrow">→</span>
        <span className="street-fx__stamp">PROPERTY OF<br /><b>DANCOLD</b></span>
      </div>
      <aside className="sidebar">
        <div className="sidebar__brand">
          <BrandMark />
          <div className="brand-motto">
            <span>Colder than all.</span>
            <strong>Always.</strong>
          </div>
          <div className="brand-scribble" aria-hidden="true">✦ // ★</div>
          <div className="brand-graffiti" aria-hidden="true">
            <span>COLD</span><b>EM!</b><i>〰〰</i>
          </div>
        </div>

        <div className="runner-ident" aria-label="The Runner, Coldem game courier">
          <img src={runnerHead} alt="" />
          <span><b>RUNNER</b><small>COURIER // 001</small></span>
          <em>01</em>
        </div>

        <nav className="sidebar__nav" aria-label="Main navigation">
          <p>Control deck</p>
          <button type="button" className={filter === "home" ? "active" : ""} onClick={() => setFilter("home")}>
            <Home size={19} /> <span>Home</span><i>✦</i>
          </button>
          <button type="button" className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>
            <Library size={19} /> <span>Library</span><small>{launcher.library?.records.length ?? 0}</small>
          </button>
          <button type="button" className={filter === "installed" ? "active" : ""} onClick={() => setFilter("installed")}>
            <HardDrive size={19} /> <span>Installed</span><small>{launcher.library?.caves.length ?? 0}</small>
          </button>
          <button type="button" className={filter === "updates" ? "active" : ""} onClick={() => setFilter("updates")}>
            <Download size={19} /> <span>Updates</span>
            {launcher.updates.length > 0 && <em>{launcher.updates.length}</em>}
          </button>
          <button type="button" onClick={() => setSettingsOpen(true)}>
            <Settings2 size={19} /> <span>Settings</span>
          </button>
        </nav>

        <div className="sidebar-signal" aria-label="Coldem system signal">
          <span><i /> SIGNAL</span><b>{playingRecord ? "PLAYING" : launcher.channel.toUpperCase()}</b>
          <em><i /><i /><i /><i /></em>
        </div>

        <div className="sidebar-mascot">
          <span className="sidebar-mascot__burst" aria-hidden="true">✶</span>
          <PetSticker kind="yin" variant={1} decorative />
          <span className="sidebar-mascot__note">Stay cold.<br />Play hard.</span>
        </div>
        <div className="sidebar__footer">
          <p>Thanks for being here.</p>
          <small>You make Coldem awesome!</small>
        </div>
      </aside>

      <section className="main-view">
        <header className="topbar">
          <div>
            <p className="eyebrow">WELCOME BACK</p>
            <div className="topbar__title-row"><h1>Ready to play?</h1><span className="topbar__runner-signal"><i /> RUNNER ONLINE</span></div>
          </div>
          <div className="topbar__tools">
            <label className="search-field">
              <Search size={17} />
              <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search games" aria-label="Search games" />
            </label>
            <button type="button" className="icon-button" onClick={() => void launcher.refresh()} aria-label="Refresh library" disabled={launcher.isRefreshing}>
              <RefreshCw size={18} className={launcher.isRefreshing ? "spin" : ""} />
            </button>
            <AppUpdateButton />
          </div>
        </header>

        <section className="coldem-status-strip" aria-label="Library status">
          <span><i className="coldem-status-strip__pulse" /> COLD SYSTEM ONLINE</span>
          <b>{installedCount} INSTALLED</b>
          <b className={updatesCount > 0 ? "coldem-status-strip__updates" : ""}>{updatesCount > 0 ? `${updatesCount} UPDATE${updatesCount === 1 ? "" : "S"} READY` : "ALL PATCHED"}</b>
          <em className={playingRecord ? "coldem-status-strip__live" : ""}>{playingRecord ? `${formatLiveSession(liveSessionSeconds)} · ${playingRecord.title}` : `DNCLD // ${filter === "home" ? "HOME BASE" : filter.toUpperCase()}`}</em>
        </section>

        <div className="content-scroll">
          {launcher.bootstrap && !launcher.bootstrap.catalogRestricted && (
            <section className="catalog-warning" role="status">
              <AlertTriangle size={18} />
              <div><strong>This developer build has no release catalog yet.</strong><p>Set COLDEM_GITHUB_REPOSITORY when building to connect the public game releases.</p></div>
            </section>
          )}

          {(launcher.libraryError || launcher.library?.warning) && (
            <section className="library-alert" role="alert">
              <AlertTriangle size={18} />
              <div>
                <strong>{launcher.library ? "Some live data is unavailable" : "The library could not be loaded"}</strong>
                <p>{launcher.libraryError || launcher.library?.warning}</p>
              </div>
              <button type="button" onClick={() => void launcher.refresh()} disabled={launcher.isRefreshing}>
                <RefreshCw size={14} className={launcher.isRefreshing ? "spin" : ""} /> Retry
              </button>
            </section>
          )}

          {featuredRecord && filter === "home" && !query && (
            <section className={`featured-hero cover-fallback--${featuredRecord.id % 8}`}>
              {featuredRecord.cover && <img className="featured-hero__image" src={featuredRecord.cover} alt="" />}
              <div className="featured-hero__wash" />
              <img className="featured-hero__runner" src={runnerActive} alt="" aria-hidden="true" />
              <div className="featured-hero__copy">
                <span className={`feature-tag ${isFeaturedPlaying ? "feature-tag--live" : ""}`}>
                  {isFeaturedPlaying ? <><i /> On air</> : <><Sparkles size={13} /> Featured</>}
                </span>
                <h2>{featuredRecord.title}</h2>
                <p>{featuredCave?.game.shortText || "A strange new world is waiting beyond the loading screen."}</p>
                <div className="featured-hero__actions">
                  <button type="button" className="hero-play-button" onClick={runFeatured} disabled={Boolean(isFeaturedBusy)}>
                    <Gamepad2 size={20} fill="currentColor" /> {featuredAction}
                  </button>
                  {featuredCave && <span><Clock3 size={15} /> {formatPlaytime(featuredCave.interaction?.secondsRun ?? featuredCave.stats.secondsRun)}</span>}
                </div>
                <div className="featured-hero__intel" aria-label="Featured game details">
                  <span><i /> {isFeaturedPlaying ? "SESSION LIVE" : featuredCave ? "IN YOUR LIBRARY" : "FRESH DROP"}</span>
                  <span>BY DANCOLD</span>
                  <span>BUILD // {featuredUpdate ? "UPDATE READY" : "CURRENT"}</span>
                </div>
                <div className="featured-hero__tape" aria-hidden="true">
                  <span>{isFeaturedPlaying ? "LIVE SIGNAL DETECTED" : "THE RUNNER BROUGHT THIS DROP"}</span>
                  <b>/// DROP 001 ///</b>
                </div>
              </div>
              <div className="featured-hero__doodles" aria-hidden="true">
                <span>PLAY<br />// COLD</span>
                <i>★</i>
                <b>••••••</b>
              </div>
              <div className="featured-hero__graffiti" aria-hidden="true">
                <span>ROCK!</span>
                <i>×</i><i>×</i><i>×</i>
                <b>NO SLEEP<br />JUST PLAY</b>
              </div>
              <div className="featured-hero__serial" aria-hidden="true">DC-{String(featuredRecord.id).slice(-5).padStart(5, "0")}<b>/// COLD ARCHIVE ///</b></div>
              <PetSticker kind="yin" variant={4} className="featured-hero__sticker" decorative />
            </section>
          )}

          <section className="library-section">
            <div className="section-heading">
              <div><p className="eyebrow">{filter === "home" || filter === "all" ? "YOUR WORLDS" : filter.toUpperCase()}</p><h2>{filter === "home" || filter === "all" ? "Game collection" : filter === "installed" ? "Ready to play" : "Available updates"}</h2></div>
              <span>{games.length} {games.length === 1 ? "game" : "games"}</span>
            </div>

            {!launcher.library ? (
              launcher.libraryError ? (
                <div className="empty-library"><AlertTriangle size={28} /><h3>Couldn't load the library</h3><p>Use Retry above. Coldem will never keep this screen spinning forever again.</p></div>
              ) : (
                <div className="library-loading"><LoaderCircle className="spin" /> Loading your games...</div>
              )
            ) : games.length === 0 ? (
              <div className="empty-library"><Gamepad2 size={28} /><h3>Nothing here yet</h3><p>{query ? "Try a different search." : "This view is all caught up."}</p></div>
            ) : (
              <div className="game-grid">
                {games.map((game) => {
                  const cave = caveByGame.get(game.id);
                  return <GameCard key={game.id} game={game} cave={cave} update={updateByGame.get(game.id)} operation={launcher.operations[game.id]} onInstall={() => void openInstall(game)} onUpdate={() => cave && safely(launcher.update(cave.id, game.id))} onPlay={() => cave && safely(launcher.play(cave.id, game.id))} onOpenPage={game.url ? () => void launcher.openExternal(game.url!) : undefined} />;
                })}
              </div>
            )}
          </section>
        </div>
      </section>

      <aside className="activity-rail">
        <section className="profile-card">
          <img src={social.snapshot?.currentUser?.avatarUrl || launcher.profile?.user.coverUrl || dancoldLogo} alt="" />
          <div><strong>{social.snapshot?.currentUser?.displayName || launcher.profile?.user.displayName || launcher.profile?.user.username}</strong><small><i /> {social.snapshot?.connection === "connected" ? "Discord connected" : "Ready to play"}</small></div>
          <button type="button" className="icon-button" aria-label="Notifications"><Bell size={18} /></button>
        </section>

        <div className="pet-pair" aria-label="Yin, the black cat, and Yang, the white cat">
          <PetSticker kind="yin" variant={1} />
          <PetSticker kind="yang" variant={1} />
        </div>
        <section className="crew-dossier" aria-label="DanCold crew dossier">
          <p>DNCLD CREW // ISSUE 01</p>
          <div><PetSticker kind="yin" variant={3} decorative /><span><b>YIN</b><small>chill signal / amber-green eyes</small></span></div>
          <div><PetSticker kind="yang" variant={2} decorative /><span><b>YANG</b><small>serious look / green eyes</small></span></div>
          <em>DEADPOOL + WOLVERINE<br />HAMSTER SUPPORT UNIT</em>
        </section>
        <div className="rail-sticker-strip" aria-hidden="true">
          <span>★</span><i>DNCLD</i><b>|||||||||||</b><em>☺</em>
        </div>

        <SocialPanel social={social} nowPlaying={playingRecord?.title} sessionSeconds={liveSessionSeconds} />

        <section className="downloads-panel">
          <div className="rail-heading"><h2>Downloads</h2>{activeOperations.length > 0 && <span>{activeOperations.length}</span>}</div>
          <p className="downloads-panel__stamp">PATCH BAY // SIGNED FILES ONLY</p>
          {activeOperations.length === 0 ? (
            <div className="downloads-empty"><Zap size={22} /><strong>All clear</strong><p>No active installs or updates.</p></div>
          ) : activeOperations.map((operation) => {
            const game = launcher.library?.records.find((record) => record.id === operation.gameId);
            return <div className="download-row" key={`${operation.kind}-${operation.gameId}`}><div className={`download-row__cover cover-fallback--${operation.gameId % 8}`}>{game?.title.slice(0, 1)}</div><div className="download-row__body"><strong>{game?.title || "Game"}</strong><small>{operation.kind === "install" ? "Installing" : operation.kind === "update" ? "Updating" : "Starting"}...</small><div><i style={{ width: `${Math.round((operation.progress ?? 0) * 100)}%` }} /></div><span>{Math.round((operation.progress ?? 0) * 100)}%</span></div></div>;
          })}
          <div className="downloads-panel__telemetry" aria-label="Patch delivery integrity">
            <span><i /> SHA-256 VERIFIED</span><b>{activeOperations.length ? "TRANSFER ACTIVE" : "BAY IDLE"}</b>
          </div>
        </section>

        <section className="rail-radio">
          <Music2 size={19} /><div><strong>Cold storage radio</strong><small>lo-fi beats / focus mode</small></div><span>♪</span>
        </section>

        <button type="button" className="logout-button" onClick={() => void launcher.logout()}><LogOut size={16} /> Leave library</button>
      </aside>

      {pendingInvite && (
        <div className="invite-backdrop" role="presentation">
          <section className="invite-landing" role="dialog" aria-modal="true" aria-labelledby="discord-join-title">
            <span className="invite-landing__stamp">DISCORD // EOS RELAY</span>
            <img src={runnerHead} alt="" className="invite-landing__runner" />
            <p className="eyebrow">SQUAD SIGNAL RECEIVED</p>
            <h2 id="discord-join-title">Robot Rock is calling.</h2>
            <p>A friend sent you a live lobby invite. Coldem will use the same EOS relay checks as a manual join.</p>
            <div className="invite-landing__code"><small>LOBBY CODE</small><strong>{pendingInvite.code}</strong></div>
            {!invitedRecord ? (
              <p className="form-error">Robot Rock is not available in this Coldem catalog yet. Refresh after its delivery release is published.</p>
            ) : isInvitedGameRunning ? (
              <p className="invite-landing__notice">Robot Rock is already running. Queue this invite and it will only join once the game is safely idle.</p>
            ) : invitedCave ? (
              <p className="invite-landing__notice">Robot Rock is installed and ready to connect.</p>
            ) : (
              <p className="invite-landing__notice">Robot Rock needs to be installed before it can join this lobby.</p>
            )}
            <div className="invite-landing__actions">
              <button type="button" className="secondary-button" onClick={() => void social.dismissJoin()}>Not now</button>
              {isInvitedGameRunning && invitePayload ? (
                <button type="button" className="primary-button" onClick={queueInvite}>Queue invite</button>
              ) : invitedCave ? (
                <button type="button" className="primary-button" onClick={joinInvite}>Join Robot Rock</button>
              ) : invitedRecord ? (
                <button type="button" className="primary-button" onClick={installAndJoinInvite}>Install &amp; join</button>
              ) : null}
            </div>
          </section>
        </div>
      )}
      {installingRecord && !installOptions && <div className="dialog-backdrop"><div className="preparing-install"><LoaderCircle className="spin" /> Preparing {installingRecord.title}...</div></div>}
      {installOptions && <InstallDialog options={installOptions} onPlan={launcher.planInstall} onInstall={performInstall} onClose={closeInstall} />}
      {settingsOpen && <SettingsDialog onClose={() => setSettingsOpen(false)} channel={launcher.channel} onChannelChange={launcher.setChannel} />}
      {actionError && <button type="button" className="error-toast" onClick={() => setActionError(null)}><span>{actionError}</span><small>Dismiss</small></button>}
    </main>
  );
}
