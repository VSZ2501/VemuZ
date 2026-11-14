<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount, tick } from "svelte";
  import { KEY_ROWS } from "./keys";
  import { CODECS } from "./bindings";
  import logoDark from "./assets/vemuz-logo.svg";
  import logoLight from "./assets/vemuz-logo-clair.svg";
  import { applyTheme, type Theme } from "./theme";
  import { detectFranchise, OTHERS } from "./franchises";
  import { fadeIn, pause, slide, withViewTransition } from "./motion";
  import { CONSOLES } from "./consoles";
  import { PAD_TYPE_LABEL, padType, readPad, type Directions, type PadType } from "./padmap";
  import { nearest, startGamepad, type PadAction } from "./gamepad";

  type Game = {
    path: string;
    title: string;
    code: string;
    db_title?: string | null;
    custom_title?: string | null;
    last_played?: number | null;
    play_time?: number;
    favorite?: boolean;
    franchise?: string | null;
  };
  type LaunchOptions = { fullscreen: boolean };
  type Settings = {
    cover_size?: string;
    theme?: string;
    pad_directions?: string;
    keep_window?: boolean;
  };
  // Émulateur tel que le backend le décrit
  type EmuInfo = {
    id: string;
    name: string;
    console: string;
    recommended: boolean;
    selected: boolean;
    dir: string | null;
    ok: boolean;
    options: LaunchOptions;
    download: boolean;
    bindings: string | null;
  };
  // Réglage déclaré par un connecteur
  type SettingDef = {
    id: string;
    label: string;
    help: string | null;
    group: string;
    advanced: boolean;
    type: "toggle" | "choice" | "range";
    options?: [unknown, string][];
    min?: number;
    max?: number;
    step?: number;
    unit?: string;
  };
  type Preset = { id: string; label: string; help: string; values: Record<string, unknown> };
  type Found = { emu: string; name: string; dir: string };

  type Tab = "consoles" | "library" | "settings";
  let view: Tab = $state("consoles");
  let games: Game[] = $state([]);
  let settings: Settings = $state({});
  let emus: EmuInfo[] = $state([]);
  // Émulateur utilisé pour la DS (seule console jouable pour l'instant)
  let dsEmu = $derived(emus.find((e) => e.console === "ds" && e.selected) ?? emus.find((e) => e.console === "ds"));
  let dsId = $derived(dsEmu?.id);
  let emuReady = $derived(!!dsEmu?.ok);
  let message = $state("");
  // Jaquettes en cache : code du jeu vers URL affichable
  let covers: Record<string, string> = $state({});

  // Renommage en cours : chemin du jeu et texte saisi
  let editing: string | null = $state(null);
  let draft = $state("");

  const errText = (e: unknown) => (typeof e === "string" ? e : String(e));
  const label = (g: Game) => g.custom_title ?? g.db_title ?? g.title;

  // Recherche (sans accents ni casse) et tri, calculés dans le frontend
  let search = $state("");
  let sort: "title" | "recent" | "time" | "franchise" = $state("title");
  // N'afficher que les favoris
  let favOnly = $state(false);
  const norm = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

  let shown = $derived.by(() => {
    const q = norm(search.trim());
    const base = favOnly ? games.filter((g) => g.favorite) : games;
    const list = q ? base.filter((g) => norm(label(g)).includes(q) || norm(g.code).includes(q)) : [...base];
    // La liste arrive déjà triée par titre ; le tri est stable
    if (sort === "recent") list.sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0));
    else if (sort === "time") list.sort((a, b) => (b.play_time ?? 0) - (a.play_time ?? 0));
    return list;
  });

  // Franchise : choisie à la main, sinon déduite du titre
  const franchiseOf = (g: Game) => g.franchise || detectFranchise(label(g)) || OTHERS;

  // Tri par franchise : groupes par ordre alphabétique, « Autres » en dernier
  let groups = $derived.by(() => {
    if (sort !== "franchise") return null;
    const map = new Map<string, Game[]>();
    for (const g of shown) {
      const f = franchiseOf(g);
      map.set(f, [...(map.get(f) ?? []), g]);
    }
    return [...map.entries()]
      .sort(([a], [b]) => (a === OTHERS ? 1 : b === OTHERS ? -1 : a.localeCompare(b, "fr")))
      .map(([name, games]) => ({ name, games }));
  });

  // Toutes les franchises présentes, proposées lors de la saisie
  let knownFranchises = $derived([...new Set(games.map(franchiseOf))].filter((f) => f !== OTHERS).sort((a, b) => a.localeCompare(b, "fr")));

  let editingFranchise = $state(false);
  let franchiseDraft = $state("");

  async function saveFranchise(g: Game) {
    editingFranchise = false;
    const value = franchiseDraft.trim();
    // Même valeur que la détection : on garde le mode automatique
    const auto = detectFranchise(label(g)) ?? "";
    try {
      games = await invoke<Game[]>("set_franchise", { path: g.path, franchise: value === auto ? "" : value });
      if (sheet?.path === g.path) sheet = games.find((x) => x.path === g.path) ?? null;
    } catch (e) {
      message = errText(e);
    }
  }

  // Dernière partie, pour le bandeau Reprendre (sans recherche ni filtre)
  let resume = $derived.by(() => {
    if (search.trim() || favOnly) return null;
    let best: Game | null = null;
    for (const g of games) if (g.last_played && (!best || g.last_played > (best.last_played ?? 0))) best = g;
    return best;
  });

  const rtf = new Intl.RelativeTimeFormat("fr", { numeric: "auto" });
  // Date relative à partir de secondes Unix : « il y a 2 heures », « hier »
  function fmtAgo(secs: number): string {
    const d = secs - Date.now() / 1000;
    const a = Math.abs(d);
    if (a < 60) return "à l'instant";
    if (a < 3600) return rtf.format(Math.round(d / 60), "minute");
    if (a < 86400) return rtf.format(Math.round(d / 3600), "hour");
    if (a < 86400 * 30) return rtf.format(Math.round(d / 86400), "day");
    return `le ${new Date(secs * 1000).toLocaleDateString("fr-FR", { dateStyle: "long" })}`;
  }

  function fmtPlay(secs: number): string {
    if (secs < 60) return "moins d'une minute";
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h > 0 ? `${h} h ${String(m).padStart(2, "0")} min` : `${m} min`;
  }

  async function toggleFavorite(g: Game) {
    try {
      games = await invoke<Game[]>("set_favorite", { path: g.path, favorite: !g.favorite });
      if (sheet?.path === g.path) sheet = games.find((x) => x.path === g.path) ?? null;
    } catch (e) {
      message = errText(e);
    }
  }

  // Taille des jaquettes : petite, moyenne, grande (largeur minimale des colonnes)
  const SIZES = [
    { id: "s", label: "Petites jaquettes", px: 110 },
    { id: "m", label: "Jaquettes moyennes", px: 140 },
    { id: "l", label: "Grandes jaquettes", px: 190 },
  ] as const;
  let sizeIndex = $derived(Math.max(0, SIZES.findIndex((z) => z.id === (settings.cover_size || "m"))));

  async function setSize(i: number) {
    const id = SIZES[Math.min(Math.max(i, 0), SIZES.length - 1)].id;
    if (id === (settings.cover_size || "m")) return;
    settings.cover_size = id;
    try {
      await invoke("set_cover_size", { size: id });
    } catch (e) {
      message = errText(e);
    }
  }

  // Ctrl + molette sur la grille : change la taille (et empêche le zoom de la page)
  function ctrlWheel(el: HTMLElement) {
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      setSize(sizeIndex + (e.deltaY < 0 ? 1 : -1));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return { destroy: () => el.removeEventListener("wheel", onWheel) };
  }

  // Thème : réglage enregistré, thème effectivement affiché (Comme Windows suit le système)
  let theme: Theme = $state(applyTheme(null));
  let shownTheme = $state<"dark" | "light">("dark");
  const logo = $derived(shownTheme === "light" ? logoLight : logoDark);

  $effect(() => {
    shownTheme = applyTheme(theme);
    if (theme !== "system") return;
    // Suit le changement de thème de Windows tant que « Comme Windows » est choisi
    const mq = matchMedia("(prefers-color-scheme: light)");
    const onChange = () => (shownTheme = applyTheme("system"));
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  });

  async function setTheme(t: Theme) {
    theme = t;
    try {
      await invoke("set_theme", { theme: t });
    } catch (e) {
      message = errText(e);
    }
  }

  function startEdit(g: Game) {
    draft = label(g);
    editing = g.path;
  }

  async function commitEdit(g: Game) {
    if (editing !== g.path) return;
    editing = null;
    if (draft.trim() === label(g)) return;
    await update("rename_game", { path: g.path, title: draft });
  }

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  onMount(async () => {
    settings = await invoke<Settings>("get_settings");
    if (settings.theme === "dark" || settings.theme === "light" || settings.theme === "system") theme = settings.theme;
    emus = await invoke<EmuInfo[]>("list_emulators");
    games = await invoke<Game[]>("list_games");
    // Fenêtre recréée après un jeu : retour sur la vue d'avant, jeu lancé sélectionné
    const resume = await invoke<{ view: string; game: string | null } | null>("get_resume");
    if (resume && (resume.view === "consoles" || resume.view === "library" || resume.view === "settings")) {
      view = resume.view;
      if (resume.game) {
        await tick();
        document.querySelector<HTMLElement>(`.card[data-path="${CSS.escape(resume.game)}"] > .box`)?.focus();
      }
    }

    const cached = await invoke<Record<string, string>>("list_covers");
    for (const [code, path] of Object.entries(cached)) covers[code] = convertFileSrc(path);
    await listen<{ code: string; path: string }>("cover-ready", (e) => {
      covers[e.payload.code] = convertFileSrc(e.payload.path);
    });
    await listen<string>("covers-error", (e) => (message = e.payload));
    await listen("games-updated", async () => (games = await invoke<Game[]>("list_games")));
  });

  // Dossiers où les émulateurs d'une console semblent installés (panneau de configuration)
  let found: Found[] = $state([]);
  let detecting = $state(false);

  async function detect(console: string) {
    detecting = true;
    try {
      found = await invoke<Found[]>("detect_emulators", { console });
    } finally {
      detecting = false;
    }
  }

  async function openDownload(emu: string) {
    try {
      await invoke("open_download_page", { emu });
    } catch (e) {
      message = errText(e);
    }
  }

  async function chooseEmuDir(e: EmuInfo | undefined = dsEmu) {
    if (!e) return;
    const { id, name } = e;
    const dir = await open({ directory: true, title: `Dossier de ${name}` });
    if (typeof dir === "string") await useDir(id, dir);
  }

  async function useDir(emu: string, dir: string) {
    try {
      emus = await invoke<EmuInfo[]>("set_emulator_dir", { emu, dir });
      const info = emus.find((e) => e.id === emu);
      const name = info?.name ?? "L'émulateur";
      if (setup) {
        // Depuis le panneau : cet émulateur devient celui de la console, puis la bibliothèque s'ouvre
        if (info && !info.selected) emus = await invoke<EmuInfo[]>("set_console_emulator", { console: setup, emu });
        setup = null;
        message = `${name} est prêt. Ajoute maintenant tes jeux.`;
        go("library");
      } else {
        message = `Dossier de ${name} enregistré.`;
      }
    } catch (e) {
      message = errText(e);
    }
  }

  // Réglages de l'émulateur : déclarés par son connecteur, lus et écrits dans son fichier de configuration
  let schema = $state<SettingDef[] | null>(null);
  let vals: Record<string, any> = $state({});
  let emuError = $state("");
  let presets: Preset[] = $state([]);
  let showAdvanced = $state(false);
  // Réglages courants regroupés par section, dans l'ordre déclaré ; les avancés à part
  let emuGroups = $derived.by(() => {
    const list: SettingDef[] = (schema ?? []).filter((s) => !s.advanced);
    return [...new Set(list.map((s) => s.group))].map((g) => ({ title: g, items: list.filter((s) => s.group === g) }));
  });
  let advItems = $derived((schema ?? []).filter((s) => s.advanced));
  // Préréglage dont toutes les valeurs correspondent aux réglages actuels
  let activePreset = $derived(presets.find((p) => Object.entries(p.values).every(([k, v]) => vals[k] === v))?.id);
  let dsEmus = $derived(emus.filter((e) => e.console === "ds"));

  async function applyPreset(p: Preset) {
    if (!dsId) return;
    try {
      vals = await invoke<Record<string, any>>("set_emulator_settings", { emu: dsId, values: p.values });
      message = "";
    } catch (e) {
      message = errText(e);
    }
  }

  async function chooseEmulator(console: string, emu: string) {
    try {
      emus = await invoke<EmuInfo[]>("set_console_emulator", { console, emu });
    } catch (e) {
      message = errText(e);
    }
  }

  // Touches du clavier : nom du bouton vers code Qt (-1 : non assignée)
  let keys: Record<string, number> | null = $state(null);
  // Bouton en attente d'une touche
  let capturing: string | null = $state(null);

  async function loadEmu(emu: string) {
    try {
      const r = await invoke<{ schema: SettingDef[]; presets: Preset[]; values: Record<string, any> }>(
        "get_emulator_settings",
        { emu },
      );
      schema = r.schema;
      presets = r.presets;
      vals = r.values;
      // Capture et affichage des touches : seul le format de melonDS est connu pour l'instant
      const fmt = emus.find((e) => e.id === emu)?.bindings;
      if (fmt && CODECS[fmt]) {
        keys = await invoke<Record<string, number>>("get_bindings", { emu, kind: "keyboard" });
        joy = await invoke<Record<string, number>>("get_bindings", { emu, kind: "pad" });
      } else {
        keys = null;
        joy = null;
      }
      emuError = "";
    } catch (e) {
      schema = null;
      keys = null;
      joy = null;
      emuError = errText(e);
    }
  }

  // Format des touches de l'émulateur choisi (capture, noms, valeurs conseillées)
  let codec = $derived(dsEmu?.bindings ? CODECS[dsEmu.bindings] : undefined);
  // Boutons et raccourcis réglables dans cet émulateur, dans l'ordre d'affichage
  const rowsFor = (map: Record<string, number>) =>
    KEY_ROWS.map((g) => ({ ...g, items: g.items.filter((i) => i.name in map) })).filter((g) => g.items.length > 0);

  // Clavier et manette dans le même emplacement : un côté modifié change l'autre
  async function refreshShared(kind: "keyboard" | "pad") {
    if (!codec?.shared || !dsId) return;
    const other = await invoke<Record<string, number>>("get_bindings", { emu: dsId, kind });
    if (kind === "keyboard") keys = other;
    else joy = other;
  }

  async function saveKeys(next: Record<string, number>) {
    try {
      keys = await invoke<Record<string, number>>("set_bindings", { emu: dsEmu?.id, kind: "keyboard", keys: next });
      await refreshShared("pad");
    } catch (e) {
      message = errText(e);
      await reloadEmu();
    }
  }

  const keyLabel = (name: string) => KEY_ROWS.flatMap((r) => r.items).find((i) => i.name === name)?.label ?? name;

  // Assigne une touche ; si elle servait déjà ailleurs, les deux boutons s'échangent
  function assignKey(name: string, code: number) {
    if (!keys) return;
    const next = { ...keys };
    const other = code < 0 ? undefined : Object.keys(next).find((n) => n !== name && next[n] === code);
    if (other) {
      next[other] = next[name];
      message = `Touche échangée avec « ${keyLabel(other)} ».`;
    } else {
      message = "";
    }
    next[name] = code;
    saveKeys(next);
  }

  // ---------- Manette dans l'émulateur ----------

  // Boutons de la manette dans l'émulateur : nom du bouton DS vers code de l'émulateur (-1 : non assigné)
  let joy: Record<string, number> | null = $state(null);
  // Bouton DS en attente d'un appui sur la manette
  let padCapturing: string | null = $state(null);
  let padKind: PadType | null = $state(null);
  let padDirs = $derived<Directions>(
    settings.pad_directions === "dpad" || settings.pad_directions === "stick" ? settings.pad_directions : "both",
  );

  // Sans touche combinée (DeSmuME), « Les deux » revient à la croix
  let dpadOn = $derived(padDirs === "dpad" || (padDirs === "both" && !codec?.bothDirections));

  async function setPadDirs(d: Directions) {
    settings.pad_directions = d;
    try {
      await invoke("set_pad_directions", { directions: d });
    } catch (e) {
      message = errText(e);
    }
  }

  function refreshPad() {
    const r = readPad();
    padKind = r ? padType(r.id) : null;
  }

  onMount(() => {
    refreshPad();
    const on = () => refreshPad();
    window.addEventListener("gamepadconnected", on);
    window.addEventListener("gamepaddisconnected", on);
    return () => {
      window.removeEventListener("gamepadconnected", on);
      window.removeEventListener("gamepaddisconnected", on);
    };
  });

  async function saveJoy(next: Record<string, number>) {
    try {
      joy = await invoke<Record<string, number>>("set_bindings", { emu: dsEmu?.id, kind: "pad", keys: next });
      await refreshShared("keyboard");
    } catch (e) {
      message = errText(e);
      await reloadEmu();
    }
  }

  // Même règle que le clavier : un bouton déjà utilisé s'échange
  function assignJoy(name: string, code: number) {
    if (!joy) return;
    const next = { ...joy };
    const other = code < 0 ? undefined : Object.keys(next).find((n) => n !== name && next[n] === code);
    if (other) {
      next[other] = next[name];
      message = `Bouton échangé avec « ${keyLabel(other)} ».`;
    } else {
      message = "";
    }
    next[name] = code;
    saveJoy(next);
  }

  // Capture : attend que tout soit relâché, puis prend le premier bouton ou stick poussé
  function capturePad(name: string) {
    refreshPad();
    if (!padKind) {
      message = "Branche une manette, puis appuie sur un de ses boutons pour qu'elle soit reconnue.";
      return;
    }
    padCapturing = name;
    let released = false;
    const loop = () => {
      if (padCapturing !== name) return;
      const r = readPad();
      if (!r) {
        padCapturing = null;
        return;
      }
      if (r.pressed.length === 0) released = true;
      else if (released) {
        const kind = padType(r.id);
        padKind = kind;
        const code = codec ? codec.padFromInput(kind, r.pressed[0]) : null;
        padCapturing = null;
        if (code === null) message = "Ce bouton n'est pas pris en charge.";
        else assignJoy(name, code);
        return;
      }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (padCapturing && e.key === "Escape") {
      e.preventDefault();
      padCapturing = null;
      return;
    }
    if (sheet && !capturing && !editingFranchise && e.key === "Escape") {
      closeSheet();
      return;
    }
    if (setup && e.key === "Escape") {
      closeSetup();
      return;
    }
    if (!capturing) return;
    e.preventDefault();
    if (e.key === "Escape") {
      capturing = null;
      return;
    }
    const code = codec ? codec.keyFromEvent(e) : null;
    if (code === null) {
      message = "Cette touche n'est pas prise en charge.";
      return;
    }
    const name = capturing;
    capturing = null;
    assignKey(name, code);
  }

  async function reloadEmu() {
    if (dsId) await loadEmu(dsId);
  }

  async function saveEmu() {
    if (!schema || !dsId) return;
    try {
      vals = await invoke<Record<string, any>>("set_emulator_settings", { emu: dsId, values: { ...vals } });
      message = "";
    } catch (e) {
      message = errText(e);
      await reloadEmu();
    }
  }

  // Recharge les réglages à chaque ouverture de la vue Réglages (ou changement d'émulateur)
  $effect(() => {
    if (view === "settings" && emuReady && dsId) loadEmu(dsId);
  });

  async function saveFullscreen(fullscreen: boolean) {
    if (!dsId) return;
    try {
      emus = await invoke<EmuInfo[]>("set_launch_options", { emu: dsId, options: { fullscreen } });
    } catch (e) {
      message = errText(e);
    }
  }

  async function saveKeepWindow(keep: boolean) {
    try {
      settings = await invoke<Settings>("set_keep_window", { keep });
    } catch (e) {
      message = errText(e);
    }
  }

  async function openEmulator() {
    message = "";
    try {
      await invoke("open_emulator", { emu: dsId });
      await invoke("sleep_window", { view, game: null });
    } catch (e) {
      message = errText(e);
    }
  }

  async function addFiles() {
    const sel = await open({
      multiple: true,
      title: "Ajouter des jeux",
      filters: [{ name: "Jeux Nintendo DS (.nds, .zip, .7z)", extensions: ["nds", "zip", "7z"] }],
    });
    if (!sel) return;
    await addPaths(Array.isArray(sel) ? sel : [sel]);
  }

  async function addFolder() {
    const dir = await open({ directory: true, title: "Dossier de jeux" });
    if (typeof dir === "string") await addPaths([dir]);
  }

  // Ajout de fichiers et de dossiers (boutons et glisser-déposer)
  let adding = $state(false);

  async function addPaths(paths: string[]) {
    if (adding) return;
    adding = true;
    message = "Ajout en cours…";
    const before = games.length;
    try {
      games = await invoke<Game[]>("add_paths", { paths });
      const n = games.length - before;
      message =
        n > 1
          ? `${n} jeux ajoutés.`
          : n === 1
            ? "1 jeu ajouté."
            : "Aucun nouveau jeu Nintendo DS trouvé dans ces fichiers.";
    } catch (e) {
      message = errText(e);
    } finally {
      adding = false;
    }
  }

  // Glisser-déposer de fichiers sur la fenêtre
  let dragging = $state(false);

  onMount(() => {
    const off = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter") dragging = true;
      else if (p.type === "leave") dragging = false;
      else if (p.type === "drop") {
        dragging = false;
        if (view === "settings") view = "library";
        addPaths(p.paths);
      }
    });
    return () => {
      off.then((f) => f());
    };
  });

  async function update(cmd: string, args: Record<string, unknown>) {
    const before = games.length;
    try {
      games = await invoke<Game[]>(cmd, args);
      const n = games.length - before;
      message = n > 0 ? `${n} jeu(x) ajouté(s).` : "";
    } catch (e) {
      message = errText(e);
    }
  }

  // Inclinaison de la jaquette à l'opposé de la souris (valeurs de -1 à 1)
  function tilt(e: MouseEvent) {
    const el = e.currentTarget as HTMLElement;
    const r = el.getBoundingClientRect();
    el.style.setProperty("--x", String(((e.clientX - r.left) / r.width) * 2 - 1));
    el.style.setProperty("--y", String(((e.clientY - r.top) / r.height) * 2 - 1));
  }

  function untilt(e: MouseEvent) {
    const el = e.currentTarget as HTMLElement;
    el.style.removeProperty("--x");
    el.style.removeProperty("--y");
  }

  // ---------- Fiche du jeu et sauvegardes ----------

  type FileInfo = { id: string; path: string; size: number; modified: number };
  type SaveInfo = {
    expected: string | null;
    save: FileInfo | null;
    states: { slot: number; modified: number }[];
    backups: FileInfo[];
  };

  let sheet: Game | null = $state(null);
  let sheetInfo: SaveInfo | null = $state(null);
  // Copie en attente de confirmation de restauration
  let confirming: string | null = $state(null);

  const fmtDate = (ms: number) => new Date(ms).toLocaleString("fr-FR", { dateStyle: "long", timeStyle: "short" });
  const fmtSize = (n: number) =>
    n >= 1048576 ? `${(n / 1048576).toFixed(1).replace(".", ",")} Mo` : `${Math.max(1, Math.round(n / 1024))} Ko`;

  async function openSheet(g: Game) {
    sheet = g;
    sheetInfo = null;
    editingFranchise = false;
    confirming = null;
    try {
      sheetInfo = await invoke<SaveInfo>("get_saves", { path: g.path });
    } catch (e) {
      message = errText(e);
    }
    await tick();
    if (padMode) focusEl(document.querySelector<HTMLElement>(".sheet button.primary, .sheet button"));
  }

  async function closeSheet() {
    const path = sheet?.path;
    sheet = null;
    confirming = null;
    await tick();
    // Rend la sélection à la jaquette du jeu
    const card = [...document.querySelectorAll<HTMLElement>(".card")].find((c) => c.dataset.path === path);
    const box = card?.querySelector<HTMLElement>(".box");
    if (padMode && box) focusEl(box);
  }

  async function exportSave() {
    if (!sheet || !sheetInfo?.save) return;
    const dest = await save({
      title: "Exporter la sauvegarde",
      defaultPath: sheetInfo.save.id,
      filters: [{ name: "Sauvegarde Nintendo DS", extensions: ["sav"] }],
    });
    if (!dest) return;
    try {
      await invoke("export_save", { path: sheet.path, dest });
      message = "Sauvegarde exportée.";
    } catch (e) {
      message = errText(e);
    }
  }

  async function exportAllSaves() {
    const dest = await save({
      title: "Exporter toutes les sauvegardes",
      defaultPath: "Sauvegardes VemuZ.zip",
      filters: [{ name: "Archive zip", extensions: ["zip"] }],
    });
    if (!dest) return;
    try {
      const n = await invoke<number>("export_all_saves", { dest });
      message = n > 1 ? `${n} sauvegardes exportées.` : "1 sauvegarde exportée.";
    } catch (e) {
      message = errText(e);
    }
  }

  async function restoreBackup(id: string) {
    if (!sheet) return;
    confirming = null;
    try {
      sheetInfo = await invoke<SaveInfo>("restore_backup", { path: sheet.path, id });
      message = "Sauvegarde restaurée. L'ancienne a été copiée dans la liste.";
    } catch (e) {
      message = errText(e);
    }
  }

  async function showInExplorer(path: string) {
    try {
      await invoke("show_in_explorer", { path });
    } catch (e) {
      message = errText(e);
    }
  }

  let backupsPath = $state("");
  $effect(() => {
    if (view === "settings" && !backupsPath)
      invoke<string>("backups_dir")
        .then((d) => (backupsPath = d))
        .catch(() => {});
  });

  // Pastille de l'onglet actif : elle glisse d'un onglet à l'autre
  let tabEls: Record<string, HTMLElement | undefined> = $state({});
  let pill = $state({ left: 4, width: 0 });
  // La largeur des onglets change une fois la police chargée : on remesure à ce moment
  let fontsReady = $state(false);
  document.fonts.ready.then(() => (fontsReady = true));
  $effect(() => {
    void fontsReady;
    const el = tabEls[view];
    if (el) pill = { left: el.offsetLeft, width: el.offsetWidth };
  });

  // ---------- Ambiance ----------

  // data-awake sur <html> : VemuZ est au premier plan. Sans lui, aucune animation en boucle ne tourne
  // (fenêtre réduite, autre application devant, jeu lancé).
  onMount(() => {
    const update = () => document.documentElement.toggleAttribute("data-awake", document.hasFocus() && !document.hidden);
    update();
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      document.removeEventListener("visibilitychange", update);
    };
  });

  // ---------- Menu des consoles ----------

  let selectedId = $state("ds");
  let selected = $derived(CONSOLES.find((c) => c.id === selectedId) ?? CONSOLES[0]);

  // Émulateur utilisé pour une console : celui choisi, sinon le premier (conseillé)
  const consoleEmu = (id: string) => emus.find((e) => e.console === id && e.selected) ?? emus.find((e) => e.console === id);

  // ready : jouable ; setup : aucun émulateur indiqué ; lost : dossier disparu ; soon : pas encore prise en charge
  function consoleState(id: string): "ready" | "setup" | "lost" | "soon" {
    const e = consoleEmu(id);
    if (!e) return "soon";
    if (e.ok) return "ready";
    return e.dir ? "lost" : "setup";
  }

  function openConsole(id: string) {
    selectedId = id;
    const state = consoleState(id);
    if (state === "ready") {
      go("library");
    } else if (state === "soon") {
      const c = CONSOLES.find((x) => x.id === id);
      message = `${c?.name ?? "Cette console"} arrivera avec un prochain émulateur.`;
    } else {
      openSetup(id);
    }
  }

  // Panneau de configuration d'une console
  let setup: string | null = $state(null);
  // Émulateur affiché dans le panneau (null : celui de la console)
  let setupEmuId: string | null = $state(null);
  let setupConsole = $derived(CONSOLES.find((c) => c.id === setup));
  let setupEmus = $derived(emus.filter((e) => e.console === setup));
  let setupEmu = $derived(setupEmus.find((e) => e.id === setupEmuId) ?? (setup ? consoleEmu(setup) : undefined));
  let setupFound = $derived(found.filter((f) => f.emu === setupEmu?.id));

  async function openSetup(id: string) {
    setup = id;
    setupEmuId = null;
    found = [];
    detect(id).then(() => {
      // Rien d'indiqué encore : on montre d'abord un émulateur déjà installé, même s'il n'est pas le conseillé
      if (setup === id && setupEmuId === null && !consoleEmu(id)?.dir && setupFound.length === 0 && found.length > 0) {
        setupEmuId = found[0].emu;
      }
    });
    await tick();
    if (padMode) focusEl(document.querySelector<HTMLElement>(".sheet button.primary, .sheet button"));
  }

  async function closeSetup() {
    const id = setup;
    setup = null;
    await tick();
    // Rend la sélection à la tuile de la console
    if (padMode && view === "consoles" && id) focusEl(document.querySelector<HTMLElement>(".tile.on"));
  }

  // ---------- Manette (mode salon) ----------

  // Vrai après une action à la manette : l'anneau suit le focus. La souris rend la main.
  let padMode = $state(false);

  $effect(() => {
    document.documentElement.toggleAttribute("data-pad", padMode);
  });

  function onMouseMove(e: MouseEvent) {
    if (padMode && (e.movementX || e.movementY)) padMode = false;
  }

  // Éléments atteignables : visibles et actifs, hors boutons de survol des jaquettes
  function focusables(): HTMLElement[] {
    // Fiche ou panneau ouvert : la sélection reste dedans
    const root = sheet || setup ? document.querySelector(".sheet") : document;
    if (!root) return [];
    const all = root.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), select:not(:disabled)");
    return [...all].filter((el) => !el.closest(".acts") && el.getClientRects().length > 0 && getComputedStyle(el).visibility !== "hidden");
  }

  function focusEl(el: HTMLElement | null) {
    if (!el) return;
    el.focus({ preventScroll: true });
    el.scrollIntoView({ block: "nearest", inline: "nearest" });
  }

  // Premier élément utile de la vue : une jaquette, sinon le premier contrôle du contenu
  function focusFirst() {
    const main = document.querySelector("main");
    const list = focusables();
    const first =
      list.find((el) => el.classList.contains("tile") && el.classList.contains("on")) ??
      list.find((el) => el.classList.contains("resume-play")) ??
      list.find((el) => el.classList.contains("box")) ??
      list.find((el) => main?.contains(el));
    focusEl(first ?? null);
  }

  // Gauche et droite modifient directement les listes et les curseurs
  function adjust(el: HTMLElement, delta: number): boolean {
    if (el instanceof HTMLSelectElement) {
      const i = Math.min(Math.max(el.selectedIndex + delta, 0), el.options.length - 1);
      if (i === el.selectedIndex) return true;
      el.selectedIndex = i;
    } else if (el instanceof HTMLInputElement && el.type === "range") {
      if (delta > 0) el.stepUp();
      else el.stepDown();
    } else {
      return false;
    }
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  }

  // Change de vue avec un fondu croisé
  // Vue demandée, connue tout de suite même si la transition n'a pas encore appliqué le changement
  let target: Tab = "consoles";
  const TAB_ORDER: Tab[] = ["consoles", "library", "settings"];
  function go(next: Tab) {
    if (target === next && view === next) return;
    // Sens du glissement : vers la gauche si on avance dans les onglets
    const from = TAB_ORDER.indexOf(target);
    document.documentElement.dataset.dir = TAB_ORDER.indexOf(next) >= from ? "next" : "prev";
    target = next;
    return withViewTransition(async () => {
      view = target;
      await tick();
    });
  }

  async function switchView(next: Tab) {
    if (view === next) return;
    await go(next);
    await tick();
    focusFirst();
  }

  function onPad(a: PadAction) {
    if (padCapturing) return;
    padMode = true;
    const cur = document.activeElement as HTMLElement | null;
    const known = cur && cur !== document.body && focusables().includes(cur);

    if (a === "back") {
      if (confirming) confirming = null;
      else if (sheet) closeSheet();
      else if (setup) closeSetup();
      else if (capturing) capturing = null;
      else if (editing) editing = null;
      else if (view === "settings") switchView("library");
      else if (view === "library") switchView("consoles");
      return;
    }
    // Saisie en cours : seule l'annulation est possible
    if (editing || capturing) return;

    if (a === "details") {
      if (!sheet && !setup && known && cur.classList.contains("box")) {
        const path = cur.closest<HTMLElement>(".card")?.dataset.path;
        const g = games.find((x) => x.path === path);
        if (g) openSheet(g);
      }
      return;
    }
    if ((sheet || setup) && (a === "prev" || a === "next" || a === "search")) return;

    if (a === "prev" || a === "next") {
      const i = TAB_ORDER.indexOf(view) + (a === "next" ? 1 : -1);
      switchView(TAB_ORDER[(i + TAB_ORDER.length) % TAB_ORDER.length]);
    } else if (a === "search") {
      const input = document.querySelector<HTMLInputElement>(".search input");
      if (input) focusEl(input);
    } else if (!known) {
      focusFirst();
    } else if (a === "accept") {
      cur.click();
    } else if ((a === "left" || a === "right") && adjust(cur, a === "left" ? -1 : 1)) {
      // valeur modifiée sur place
    } else {
      focusEl(nearest(cur, focusables(), a as "up" | "down" | "left" | "right"));
    }
  }

  onMount(() => startGamepad(onPad));

  // Jeu en cours de lancement : la jaquette grossit et l'écran s'assombrit avant la réduction
  let launching: string | null = $state(null);

  // Copie de la jaquette qui vole vers le centre de l'écran au lancement
  let ghost: {
    x: number;
    y: number;
    w: number;
    h: number;
    tx: number;
    ty: number;
    from: number;
    src?: string;
    letter: string;
    title: string;
  } | null = $state(null);

  // Lancement en environ 2,5 s : la jaquette vole au centre, melonDS démarre en fond vers 1 s,
  // VemuZ se réduit à la fin (sa fenêtre arrive à peu près à ce moment-là)
  const SPAWN_AT = 1000;
  const LAUNCH_TOTAL = 2500;

  async function launch(g: Game) {
    if (launching) return;
    message = "";
    const card = [...document.querySelectorAll<HTMLElement>(".card")].find((c) => c.dataset.path === g.path);
    const r = sheet ? undefined : card?.querySelector(".box")?.getBoundingClientRect();
    // Sans jaquette visible (fiche, bandeau) : la copie apparaît directement au centre
    const w = r?.width ?? 140;
    const h = r?.height ?? 126;
    const x = r?.left ?? innerWidth / 2 - w / 2;
    const y = r?.top ?? innerHeight / 2 - h / 2;
    ghost = {
      x,
      y,
      w,
      h,
      tx: innerWidth / 2 - (x + w / 2),
      ty: innerHeight / 2 - (y + h / 2),
      from: r ? 1 : 0.6,
      src: covers[g.code],
      letter: label(g).trim().charAt(0).toUpperCase(),
      title: label(g),
    };
    launching = g.path;
    const start = performance.now();
    await pause(SPAWN_AT);
    try {
      await invoke("launch_game", { path: g.path });
    } catch (e) {
      launching = null;
      ghost = null;
      message = errText(e);
      return;
    }
    await pause(Math.max(0, LAUNCH_TOTAL - (performance.now() - start)));
    // Ferme ou réduit la fenêtre selon le réglage (si fermée, la suite ne s'exécute pas)
    await invoke("sleep_window", { view, game: g.path }).catch(() => {});
    launching = null;
    ghost = null;
    games = await invoke<Game[]>("list_games");
  }
</script>

<!-- Une ligne de réglage d'émulateur, selon son type -->
{#snippet settingRow(s: SettingDef)}
  <label class="row">
    <span class="lbl">{s.label}{#if s.help}<small>{s.help}</small>{/if}</span>
    {#if s.type === "toggle"}
      <input type="checkbox" bind:checked={vals[s.id]} onchange={saveEmu} />
    {:else if s.type === "choice"}
      <select bind:value={vals[s.id]} onchange={saveEmu}>
        {#each s.options ?? [] as [v, l]}<option value={v}>{l}</option>{/each}
      </select>
    {:else}
      <span class="range">
        <input type="range" min={s.min} max={s.max} step={s.step} bind:value={vals[s.id]} onchange={saveEmu} />
        <span class="val">{vals[s.id]} {s.unit}</span>
      </span>
    {/if}
  </label>
{/snippet}

{#snippet card(g: Game, i: number)}
  <li class="card" class:launched={launching === g.path && !!ghost} data-path={g.path} style:--i={Math.min(i, 18)}>
    <button
      class="box"
      onclick={() => launch(g)}
      onmousemove={tilt}
      onmouseleave={untilt}
      disabled={!emuReady}
      title="{g.code}&#10;{g.path}"
    >
      {#if covers[g.code]}
        <img src={covers[g.code]} alt={label(g)} loading="lazy" decoding="async" />
      {:else}
        <span class="ph" aria-hidden="true">{label(g).trim().charAt(0).toUpperCase()}</span>
      {/if}
    </button>
    {#if g.favorite}<span class="fav" title="Favori"><svg viewBox="0 0 16 16"><path d="M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z" /></svg></span>{/if}
    {#if editing === g.path}
      <input
        class="edit"
        bind:value={draft}
        use:focusSelect
        onblur={() => commitEdit(g)}
        onkeydown={(e) => {
          if (e.key === "Enter") commitEdit(g);
          else if (e.key === "Escape") editing = null;
        }}
      />
    {:else}
      <span class="title" title={label(g)}>{label(g)}</span>
    {/if}
    <div class="acts">
      <button
        class="icon"
        class:on={g.favorite}
        onclick={() => toggleFavorite(g)}
        title={g.favorite ? "Retirer des favoris" : "Ajouter aux favoris"}
        aria-label={g.favorite ? "Retirer des favoris" : "Ajouter aux favoris"}
      >
        <svg viewBox="0 0 16 16"><path d="M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z" /></svg>
      </button>
      <button class="icon" onclick={() => openSheet(g)} title="Fiche du jeu et sauvegardes" aria-label="Fiche du jeu">
        <svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="6" /><path d="M8 7.5v4M8 4.8v.1" /></svg>
      </button>
      <button class="icon" onclick={() => startEdit(g)} title="Renommer" aria-label="Renommer">
        <svg viewBox="0 0 16 16"><path d="M3 13l1-3.5L10.5 3 13 5.5 6.5 12z" /></svg>
      </button>
      <button
        class="icon"
        onclick={() => update("remove_game", { path: g.path })}
        title="Retirer de la liste"
        aria-label="Retirer de la liste"
      >
        <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>
  </li>
{/snippet}

<svelte:window onkeydown={onKeyDown} onmousemove={onMouseMove} />

<header>
  <div class="brand">
    <img src={logo} alt="" width="30" height="30" />
    <span>VemuZ</span>
  </div>
  <nav>
    <span class="tab-pill" style:left="{pill.left}px" style:width="{pill.width}px" aria-hidden="true"></span>
    <button class="tab" class:active={view === "consoles"} bind:this={tabEls.consoles} onclick={() => go("consoles")}>Consoles</button>
    <button class="tab" class:active={view === "library"} bind:this={tabEls.library} onclick={() => go("library")}>Bibliothèque</button>
    <button class="tab" class:active={view === "settings"} bind:this={tabEls.settings} onclick={() => go("settings")}>Réglages</button>
  </nav>
</header>

<main>
  {#if message}
    <div class="msg" role="status" transition:slide={{ y: -20, duration: 280 }}>
      <span>{message}</span>
      <button class="icon" title="Fermer" aria-label="Fermer" onclick={() => (message = "")}>
        <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>
  {/if}

  {#if view === "consoles"}
    <div class="home">
      <div class="home-head">
        {#key selected.id}
          <h1 class="home-title" in:slide={{ y: 14, duration: 260 }}>{selected.name}</h1>
        {/key}
        <p class="home-sub">
          Sortie en {selected.year}.
          {#if consoleState(selected.id) === "ready"}
            Jouable avec {consoleEmu(selected.id)?.name}, {games.length} {games.length > 1 ? "jeux" : "jeu"} dans ta bibliothèque.
          {:else if consoleState(selected.id) === "lost"}
            {consoleEmu(selected.id)?.name} est introuvable dans son dossier : ouvre la console pour le retrouver.
          {:else if consoleState(selected.id) === "setup"}
            Ouvre la console pour choisir un émulateur et commencer à jouer.
          {:else}
            Prise en charge prévue avec un prochain émulateur.
          {/if}
        </p>
      </div>
      <ul class="shelf">
        {#each CONSOLES as c, i (c.id)}
          <li style:--i={i}>
            <button
              class="tile"
              class:on={c.id === selectedId}
              class:soon={consoleState(c.id) === "soon"}
              onmouseenter={() => (selectedId = c.id)}
              onfocus={() => (selectedId = c.id)}
              onclick={() => openConsole(c.id)}
            >
              <svg class="tile-art" viewBox="0 0 64 48" aria-hidden="true">{@html c.draw}</svg>
              <span class="tile-name">{c.name}</span>
              {#if consoleState(c.id) === "ready"}
                <span class="tile-state">{games.length} {games.length > 1 ? "jeux" : "jeu"}</span>
              {:else if consoleState(c.id) === "soon"}
                <span class="tile-state">Bientôt</span>
              {:else}
                <span class="tile-state todo">{consoleState(c.id) === "lost" ? "Émulateur introuvable" : "À configurer"}</span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {:else if view === "library"}
    <div class="lib-head">
      <h1>Nintendo DS</h1>
      {#if games.length > 0}
        <span class="count">
          {shown.length === games.length && !favOnly ? games.length : `${shown.length} sur ${games.length}`}
          {games.length > 1 ? "jeux" : "jeu"}
        </span>
      {/if}
    </div>

    {#if games.length === 0}
      <div class="empty">
        <p class="empty-title">Ta bibliothèque est vide</p>
        <p>Ajoute tes fichiers .nds, .zip ou .7z, ou tout un dossier d'un coup. Tu peux aussi les glisser dans la fenêtre.</p>
        <div class="empty-acts">
          <button class="primary" onclick={addFiles}>Ajouter des jeux</button>
          <button onclick={addFolder}>Ajouter un dossier</button>
        </div>
      </div>
    {:else}
      <div class="bar">
        <button class="primary" onclick={addFiles}>Ajouter des jeux</button>
        <button onclick={addFolder}>Ajouter un dossier</button>
        <span class="spacer"></span>
        <button class="fav-toggle" class:on={favOnly} aria-pressed={favOnly} onclick={() => (favOnly = !favOnly)}>
          <svg viewBox="0 0 16 16"><path d="M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z" /></svg>
          Favoris
        </button>
        <label class="search">
          <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5" /><path d="M10.5 10.5L14 14" /></svg>
          <input type="search" placeholder="Rechercher" bind:value={search} />
        </label>
        <select bind:value={sort} aria-label="Tri">
          <option value="title">Titre (A-Z)</option>
          <option value="recent">Dernier lancé</option>
          <option value="time">Temps de jeu</option>
          <option value="franchise">Franchise</option>
        </select>
        <div class="sizes" role="group" aria-label="Taille des jaquettes">
          {#each SIZES as z, i (z.id)}
            <button class="size" class:on={i === sizeIndex} aria-pressed={i === sizeIndex} title={z.label} aria-label={z.label} onclick={() => setSize(i)}>
              {#if z.id === "s"}
                <svg viewBox="0 0 16 16"><rect x="2" y="2" width="3" height="3" /><rect x="6.5" y="2" width="3" height="3" /><rect x="11" y="2" width="3" height="3" /><rect x="2" y="6.5" width="3" height="3" /><rect x="6.5" y="6.5" width="3" height="3" /><rect x="11" y="6.5" width="3" height="3" /><rect x="2" y="11" width="3" height="3" /><rect x="6.5" y="11" width="3" height="3" /><rect x="11" y="11" width="3" height="3" /></svg>
              {:else if z.id === "m"}
                <svg viewBox="0 0 16 16"><rect x="2" y="2" width="5" height="5" /><rect x="9" y="2" width="5" height="5" /><rect x="2" y="9" width="5" height="5" /><rect x="9" y="9" width="5" height="5" /></svg>
              {:else}
                <svg viewBox="0 0 16 16"><rect x="2" y="2" width="12" height="12" /></svg>
              {/if}
            </button>
          {/each}
        </div>
      </div>

      {#if !emuReady}
        <p class="note">
          {consoleState("ds") === "lost"
            ? `${dsEmu?.name} est introuvable : retrouve son dossier pour lancer un jeu.`
            : "Choisis un émulateur pour lancer tes jeux."}
          <button onclick={() => openSetup("ds")}>Configurer</button>
        </p>
      {/if}

      {#if resume}
        <div class="resume">
          <div class="resume-cover">
            {#if covers[resume.code]}
              <img src={covers[resume.code]} alt="" />
            {:else}
              <span>{label(resume).trim().charAt(0).toUpperCase()}</span>
            {/if}
          </div>
          <div class="resume-text">
            <span class="soft">Dernière partie, {fmtAgo(resume.last_played ?? 0)}</span>
            <span class="resume-title">{label(resume)}</span>
          </div>
          <button class="primary resume-play" onclick={() => resume && launch(resume)} disabled={!emuReady}>Reprendre</button>
        </div>
      {/if}

      {#if shown.length === 0 && favOnly && !search.trim()}
        <div class="empty">
          <p class="empty-title">Aucun favori pour l'instant</p>
          <p>Ajoute un jeu aux favoris avec l'étoile sur sa jaquette, ou depuis sa fiche.</p>
        </div>
      {:else if shown.length === 0}
        <div class="empty">
          <p class="empty-title">Aucun jeu ne correspond à « {search.trim()} »</p>
          <p>Vérifie l'orthographe, ou cherche par code de jeu (par exemple AMCE).</p>
        </div>
      {:else}
      {#if groups}
        {#each groups as grp (grp.name)}
          <div class="group-head">
            <h2>{grp.name}</h2>
            <span class="count">{grp.games.length} {grp.games.length > 1 ? "jeux" : "jeu"}</span>
          </div>
          <ul class="grid" style:--cover="{SIZES[sizeIndex].px}px" use:ctrlWheel>
            {#each grp.games as g, i (g.path)}{@render card(g, i)}{/each}
          </ul>
        {/each}
      {:else}
        <ul class="grid" style:--cover="{SIZES[sizeIndex].px}px" use:ctrlWheel>
          {#each shown as g, i (g.path)}{@render card(g, i)}{/each}
        </ul>
      {/if}
      {/if}
    {/if}
  {:else}
    <div class="settings">
      <h1>Réglages</h1>

      <section>
        <h2>Nintendo DS</h2>
        <div class="rows">
          <label class="row">
            <span class="lbl">
              Émulateur
              <small>Celui qui lance tes jeux DS. Ses réglages s'affichent plus bas.</small>
            </span>
            <select value={dsId} onchange={(e) => chooseEmulator("ds", e.currentTarget.value)}>
              {#each dsEmus as e (e.id)}
                <option value={e.id}>{e.name}{e.recommended ? " (conseillé)" : ""}</option>
              {/each}
            </select>
          </label>
          <div class="row">
            <span class="lbl">
              Dossier
              <small><code>{dsEmu?.dir ?? "non défini"}</code></small>
            </span>
            <button onclick={() => chooseEmuDir()}>Choisir le dossier</button>
          </div>
          <p class="status" class:ok={emuReady} class:ko={!emuReady}>
            <span class="dot"></span>{emuReady
              ? "Exécutable trouvé"
              : `Exécutable introuvable : choisis le dossier de ${dsEmu?.name ?? "l'émulateur"}.`}
          </p>
        </div>
      </section>

      <section>
        <h2>Lancement</h2>
        <div class="rows">
          <label class="row">
            <span class="lbl">Plein écran</span>
            <input
              type="checkbox"
              checked={dsEmu?.options.fullscreen ?? false}
              onchange={(e) => saveFullscreen(e.currentTarget.checked)}
            />
          </label>
          <div class="row">
            <span class="lbl">
              Configuration avancée
              <small>Ouvre {dsEmu?.name ?? "l'émulateur"} seul, par exemple pour régler une manette.</small>
            </span>
            <button onclick={openEmulator} disabled={!emuReady}>Ouvrir {dsEmu?.name ?? "l'émulateur"}</button>
          </div>
          <label class="row">
            <span class="lbl">
              Libérer la mémoire pendant le jeu
              <small>
                VemuZ se ferme pendant la partie et revient quand tu quittes l'émulateur. Son icône près de l'horloge le rouvre
                à tout moment.
              </small>
            </span>
            <input type="checkbox" checked={!settings.keep_window} onchange={(e) => saveKeepWindow(!e.currentTarget.checked)} />
          </label>
        </div>
      </section>

      <section>
        <h2>Apparence</h2>
        <div class="rows">
          <div class="row">
            <span class="lbl">Thème</span>
            <div class="seg" role="group" aria-label="Thème">
              <button class:on={theme === "dark"} aria-pressed={theme === "dark"} onclick={() => setTheme("dark")}>Sombre</button>
              <button class:on={theme === "light"} aria-pressed={theme === "light"} onclick={() => setTheme("light")}>Clair</button>
              <button class:on={theme === "system"} aria-pressed={theme === "system"} onclick={() => setTheme("system")}>Comme Windows</button>
            </div>
          </div>
        </div>
      </section>

      <section>
        <h2>Sauvegardes</h2>
        <div class="rows">
          <div class="row">
            <span class="lbl">
              Exporter toutes les sauvegardes
              <small>Une archive .zip avec le fichier .sav de chaque jeu, pour les garder ailleurs.</small>
            </span>
            <button onclick={exportAllSaves}>Exporter…</button>
          </div>
          <div class="row">
            <span class="lbl">
              Copies automatiques
              <small>
                Avant et après chaque partie, VemuZ copie la sauvegarde si elle a changé, et garde les 10 dernières par jeu.
              </small>
              {#if backupsPath}<small><code>{backupsPath}</code></small>{/if}
            </span>
            <button onclick={() => showInExplorer(backupsPath)} disabled={!backupsPath}>Ouvrir le dossier</button>
          </div>
        </div>
      </section>

      {#if emuReady}
        {#if emuError}
          <section>
            <h2>Émulation</h2>
            <div class="rows"><p class="status ko"><span class="dot"></span>{emuError}</p></div>
          </section>
        {:else if schema}
          {#if presets.length > 0}
            <section>
              <h2>Préréglages</h2>
              <div class="rows">
                <div class="row">
                  <span class="lbl">
                    Réglage rapide
                    <small>{presets.find((p) => p.id === activePreset)?.help ?? "Réglages personnalisés."}</small>
                  </span>
                  <div class="seg" role="group" aria-label="Préréglages">
                    {#each presets as p (p.id)}
                      <button class:on={activePreset === p.id} aria-pressed={activePreset === p.id} onclick={() => applyPreset(p)}>
                        {p.label}
                      </button>
                    {/each}
                  </div>
                </div>
              </div>
            </section>
          {/if}

          <!-- Réglages déclarés par le connecteur, une section par groupe -->
          {#each emuGroups as grp (grp.title)}
            <section>
              <h2>{grp.title}</h2>
              <div class="rows">
                {#each grp.items as s (s.id)}{@render settingRow(s)}{/each}
              </div>
            </section>
          {/each}

          {#if keys}
            <section>
              <h2>Touches du clavier</h2>
              <div class="rows">
                <p class="hint">
                  Clique sur un bouton puis appuie sur la touche voulue (Échap annule).
                  {#if codec?.shared}
                    {dsEmu?.name} ne garde qu'une commande par bouton : choisir une touche remplace le bouton de manette.
                  {/if}
                  {#if codec?.keyNote}{codec.keyNote}{/if}
                </p>
                {#each rowsFor(keys) as group}
                  <h3>{group.title}</h3>
                  <div class="keys">
                    {#each group.items as it}
                      <div class="krow">
                        <span class="klabel">{it.label}</span>
                        <button class="kbtn" class:wait={capturing === it.name} onclick={() => (capturing = it.name)}>
                          {capturing === it.name ? "Appuie sur une touche…" : codec?.keyName(keys[it.name] ?? -1)}
                        </button>
                        <button
                          class="icon"
                          title="Désassigner"
                          aria-label="Désassigner"
                          onclick={() => assignKey(it.name, -1)}
                          disabled={(keys[it.name] ?? -1) < 0}
                        >
                          <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
                        </button>
                      </div>
                    {/each}
                  </div>
                {/each}
                <div class="kreset">
                  <button onclick={() => codec && saveKeys({ ...codec.keySuggested })}>Revenir aux valeurs conseillées</button>
                </div>
              </div>
            </section>
          {/if}

          {#if joy}
            <section>
              <h2>Manette dans {dsEmu?.name}</h2>
              <div class="rows">
                <p class="status" class:ok={!!padKind}>
                  <span class="dot"></span>{padKind ? `${PAD_TYPE_LABEL[padKind]} détectée` : "Aucune manette détectée : branche-la et appuie sur un bouton."}
                </p>
                <p class="hint">
                  Clique sur un bouton de la DS, puis appuie sur le bouton voulu de ta manette ou pousse un stick (Échap
                  annule).
                  {#if codec?.shared}
                    DeSmuME ne garde qu'une commande par bouton : choisir un bouton de manette remplace la touche du
                    clavier. Il lit la manette avec DirectInput : vérifie une fois avec « Tester dans DeSmuME ».
                  {:else}
                    melonDS utilise la première manette branchée. Les numéros de boutons suivent les pilotes habituels de
                    melonDS : vérifie une fois avec « Tester dans melonDS ».
                  {/if}
                </p>
                {#each rowsFor(joy) as group}
                  <h3>{group.title}</h3>
                  <div class="keys">
                    {#each group.items as it}
                      <div class="krow">
                        <span class="klabel">{it.label}</span>
                        <button class="kbtn" class:wait={padCapturing === it.name} onclick={() => capturePad(it.name)}>
                          {padCapturing === it.name ? "Appuie sur la manette…" : codec?.padName(padKind ?? "xbox", joy[it.name] ?? -1)}
                        </button>
                        <button
                          class="icon"
                          title="Désassigner"
                          aria-label="Désassigner"
                          onclick={() => assignJoy(it.name, -1)}
                          disabled={(joy[it.name] ?? -1) < 0}
                        >
                          <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
                        </button>
                      </div>
                    {/each}
                  </div>
                {/each}
                <div class="row">
                  <span class="lbl">Directions<small>Utilisées par « Valeurs conseillées ».</small></span>
                  <div class="seg" role="group" aria-label="Directions">
                    <button class:on={dpadOn} aria-pressed={dpadOn} onclick={() => setPadDirs("dpad")}>Croix</button>
                    <button class:on={padDirs === "stick"} aria-pressed={padDirs === "stick"} onclick={() => setPadDirs("stick")}>Stick gauche</button>
                    {#if codec?.bothDirections}
                      <button class:on={padDirs === "both"} aria-pressed={padDirs === "both"} onclick={() => setPadDirs("both")}>Les deux</button>
                    {/if}
                  </div>
                </div>
                <div class="kreset sheet-acts">
                  <button onclick={() => codec && saveJoy(codec.padSuggested(padKind ?? "xbox", padDirs))}>Valeurs conseillées</button>
                  <button onclick={openEmulator} disabled={!emuReady}>Tester dans {dsEmu?.name}</button>
                </div>
              </div>
            </section>
          {/if}

          {#if advItems.length > 0}
            <section>
              <h2>Avancé</h2>
              <div class="rows">
                <div class="row">
                  <span class="lbl">
                    Réglages avancés
                    <small>Pour ajuster finement {dsEmu?.name}. Les valeurs par défaut conviennent à la plupart des jeux.</small>
                  </span>
                  <button aria-expanded={showAdvanced} onclick={() => (showAdvanced = !showAdvanced)}>
                    {showAdvanced ? "Masquer" : "Afficher"}
                  </button>
                </div>
                {#if showAdvanced}
                  {#each advItems as s (s.id)}{@render settingRow(s)}{/each}
                {/if}
              </div>
            </section>
          {/if}
        {/if}
      {/if}
    </div>
  {/if}
</main>

<!-- Configuration d'une console : choix de l'émulateur et de son dossier -->
{#if setup && setupConsole}
  <div class="sheet-back" role="presentation" onclick={closeSetup} transition:fadeIn></div>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-label="Configurer {setupConsole.name}"
    tabindex="-1"
    transition:slide={{ x: 440, duration: 360 }}
  >
    <div class="sheet-head setup-head">
      <svg class="setup-art" viewBox="0 0 64 48" aria-hidden="true">{@html setupConsole.draw}</svg>
      <div class="sheet-id">
        <h2>{setupConsole.name}</h2>
        <p class="hint">Configuration</p>
      </div>
      <button class="icon sheet-close" title="Fermer" aria-label="Fermer" onclick={closeSetup}>
        <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>

    <p class="lead">
      Pour jouer, VemuZ s'appuie sur un émulateur gratuit que tu installes toi-même, puis tu lui indiques son dossier.
    </p>

    {#if setupEmus.length > 1}
      <div class="seg" role="group" aria-label="Émulateur">
        {#each setupEmus as e (e.id)}
          <button class:on={setupEmu?.id === e.id} aria-pressed={setupEmu?.id === e.id} onclick={() => (setupEmuId = e.id)}>
            {e.name}{e.recommended ? " (conseillé)" : ""}
          </button>
        {/each}
      </div>
    {/if}

    {#if setupEmu}
      {#if setupEmu.dir && !setupEmu.ok}
        <p class="status ko">
          <span class="dot"></span>{setupEmu.name} n'est plus dans <code>{setupEmu.dir}</code>.
        </p>
      {/if}

      {#if detecting}
        <p class="hint">Recherche de {setupEmu.name} sur ton ordinateur…</p>
      {:else if setupFound.length > 0}
        <h3>{setupFound.length > 1 ? `${setupEmu.name} est installé à plusieurs endroits` : `${setupEmu.name} est déjà installé`}</h3>
        <ul class="found">
          {#each setupFound as d, i (d.dir)}
            <li>
              <code>{d.dir}</code>
              <button class:primary={i === 0} onclick={() => useDir(d.emu, d.dir)}>Utiliser</button>
            </li>
          {/each}
        </ul>
        <p class="hint">
          {setupFound.length > 1 ? "Aucun ne convient ?" : "Ce n'est pas le bon ?"}
          <button class="link" onclick={() => chooseEmuDir(setupEmu)}>Choisis un autre dossier</button>.
        </p>
      {:else}
        <h3>Installer {setupEmu.name}</h3>
        <ol class="steps">
          {#if setupEmu.download}
            <li>
              <span>Télécharge la version Windows sur le site officiel de {setupEmu.name}.</span>
              <button class="primary" onclick={() => setupEmu && openDownload(setupEmu.id)}>Ouvrir le site</button>
            </li>
          {/if}
          <li>
            <span>Décompresse le fichier dans un dossier, par exemple <code>Documents\{setupEmu.name}</code>.</span>
          </li>
          <li>
            <span>Indique ce dossier à VemuZ.</span>
            <span class="step-acts">
              <button onclick={() => setup && detect(setup)}>Rechercher à nouveau</button>
              <button onclick={() => chooseEmuDir(setupEmu)}>Choisir le dossier</button>
            </span>
          </li>
        </ol>
      {/if}
    {/if}

    <p class="legal">VemuZ ne fournit ni jeux, ni BIOS, ni firmware.</p>
  </div>
{/if}

{#if sheet}
  <div class="sheet-back" role="presentation" onclick={closeSheet} transition:fadeIn></div>
  <div class="sheet" role="dialog" aria-modal="true" aria-label={label(sheet)} tabindex="-1" transition:slide={{ x: 440, duration: 360 }}>
    <div class="sheet-head">
      <div class="sheet-cover">
        {#if covers[sheet.code]}
          <img src={covers[sheet.code]} alt="" />
        {:else}
          <span>{label(sheet).trim().charAt(0).toUpperCase()}</span>
        {/if}
      </div>
      <div class="sheet-id">
        <h2>{label(sheet)}</h2>
        <p class="hint">{sheet.code || "Code inconnu"}</p>
        <div class="sheet-acts">
          <button class="primary" onclick={() => sheet && launch(sheet)} disabled={!emuReady}>Lancer</button>
          <button class="fav-toggle" class:on={sheet.favorite} aria-pressed={!!sheet.favorite} onclick={() => sheet && toggleFavorite(sheet)}>
            <svg viewBox="0 0 16 16"><path d="M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z" /></svg>
            {sheet.favorite ? "Favori" : "Ajouter aux favoris"}
          </button>
        </div>
      </div>
      <button class="icon sheet-close" title="Fermer" aria-label="Fermer" onclick={closeSheet}>
        <svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>

    <h3>Parties</h3>
    <ul class="sheet-list">
      <li>
        <span>Franchise</span>
        {#if editingFranchise}
          <span class="sheet-acts">
            <input
              class="fr-input"
              list="franchises"
              bind:value={franchiseDraft}
              use:focusSelect
              placeholder="Automatique"
              onkeydown={(e) => {
                if (e.key === "Enter") sheet && saveFranchise(sheet);
                else if (e.key === "Escape") {
                  e.stopPropagation();
                  editingFranchise = false;
                }
              }}
            />
            <button class="primary" onclick={() => sheet && saveFranchise(sheet)}>OK</button>
          </span>
          <datalist id="franchises">
            {#each knownFranchises as f (f)}<option value={f}></option>{/each}
          </datalist>
        {:else}
          <span class="sheet-acts">
            <span class="soft">{franchiseOf(sheet)}{sheet.franchise ? "" : " (automatique)"}</span>
            <button
              onclick={() => {
                if (!sheet) return;
                franchiseDraft = sheet.franchise ?? "";
                editingFranchise = true;
              }}>Modifier</button
            >
          </span>
        {/if}
      </li>
      <li>
        <span>Dernière partie</span>
        <span class="soft">{sheet.last_played ? fmtDate(sheet.last_played * 1000) : "Jamais lancé"}</span>
      </li>
      <li>
        <span>Temps de jeu</span>
        <span class="soft">{sheet.play_time ? fmtPlay(sheet.play_time) : "Aucun"}</span>
      </li>
    </ul>

    <h3>Sauvegarde</h3>
    {#if !sheetInfo}
      <p class="hint">Lecture…</p>
    {:else if sheetInfo.save}
      <p class="sheet-line">
        <span>Modifiée le {fmtDate(sheetInfo.save.modified)}</span>
        <span class="soft">{fmtSize(sheetInfo.save.size)}</span>
      </p>
      <div class="sheet-acts">
        <button onclick={exportSave}>Exporter…</button>
        <button onclick={() => sheetInfo?.save && showInExplorer(sheetInfo.save.path)}>Afficher dans l'Explorateur</button>
      </div>
    {:else}
      <p class="hint">Aucune sauvegarde pour l'instant. Elle apparaîtra après ta première partie.</p>
    {/if}

    {#if sheetInfo && sheetInfo.states.length > 0}
      <h3>Sauvegardes rapides</h3>
      <ul class="sheet-list">
        {#each sheetInfo.states as st (st.slot)}
          <li><span>Emplacement {st.slot}</span><span class="soft">{fmtDate(st.modified)}</span></li>
        {/each}
      </ul>
    {/if}

    {#if sheetInfo}
      <h3>Copies automatiques</h3>
      {#if sheetInfo.backups.length === 0}
        <p class="hint">VemuZ copie la sauvegarde avant et après chaque partie, et garde les 10 dernières.</p>
      {:else}
        <ul class="sheet-list">
          {#each sheetInfo.backups as b (b.id)}
            <li>
              {#if confirming === b.id}
                <span>Remplacer la sauvegarde actuelle par celle du {fmtDate(b.modified)} ? Elle sera copiée avant.</span>
                <span class="sheet-acts">
                  <button class="primary" onclick={() => restoreBackup(b.id)}>Restaurer</button>
                  <button onclick={() => (confirming = null)}>Annuler</button>
                </span>
              {:else}
                <span>{fmtDate(b.modified)}</span>
                <span class="sheet-acts">
                  <span class="soft">{fmtSize(b.size)}</span>
                  <button onclick={() => (confirming = b.id)}>Restaurer</button>
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
{/if}

{#if dragging}
  <div class="drop" aria-hidden="true" transition:fadeIn={{ duration: 150 }}>
    <div class="drop-zone">
      <p class="drop-title">Dépose tes jeux ici</p>
      <p>Fichiers .nds, .zip ou .7z, ou des dossiers entiers.</p>
    </div>
  </div>
{/if}

{#if launching}
  <div class="launch-veil" aria-hidden="true" in:fadeIn={{ duration: 500 }}></div>
{/if}
{#if ghost}
  <div
    class="ghost"
    aria-hidden="true"
    style:left="{ghost.x}px"
    style:top="{ghost.y}px"
    style:width="{ghost.w}px"
    style:height="{ghost.h}px"
    style:--tx="{ghost.tx}px"
    style:--ty="{ghost.ty}px"
    style:--from={ghost.from}
  >
    {#if ghost.src}<img src={ghost.src} alt="" />{:else}<span>{ghost.letter}</span>{/if}
  </div>
  <div class="launch-caption" aria-live="polite" style:top="{innerHeight / 2 + ghost.h * 0.8 + 32}px">
    <span class="launch-title">{ghost.title}</span>
    <span class="launch-bar"><span></span></span>
  </div>
{/if}

{#if padMode}
  <footer class="pad-help" aria-hidden="true" transition:slide={{ y: 24, duration: 280 }}>
    <span><kbd>A</kbd>Valider</span>
    <span><kbd>B</kbd>Retour</span>
    <span><kbd>LB</kbd><kbd>RB</kbd>Onglets</span>
    {#if view === "library" && games.length > 0 && !sheet}<span><kbd>X</kbd>Fiche</span><span><kbd>Y</kbd>Rechercher</span>{/if}
  </footer>
{/if}

<style>
  :global(#app) {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  /* En-tête : logo et onglets */
  header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 28px;
    padding: 14px 28px;
    border-bottom: 1px solid var(--surface-2);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    font-size: var(--fs-3);
    letter-spacing: -0.01em;
  }
  nav {
    position: relative;
    display: flex;
    gap: 4px;
    padding: 4px;
    background: var(--surface);
    border-radius: 999px;
  }
  /* Pastille de l'onglet actif, sous les boutons */
  .tab-pill {
    position: absolute;
    top: 4px;
    bottom: 4px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--accent);
    transition:
      left var(--mid) var(--ease),
      width var(--mid) var(--ease);
  }
  .tab {
    position: relative;
  }
  .tab {
    background: transparent;
    color: var(--text-soft);
    font-weight: 500;
    padding: 6px 18px;
  }
  .tab:hover:not(:disabled) {
    background: var(--surface-2);
    border-color: transparent;
    color: var(--text);
  }
  .tab.active,
  .tab.active:hover:not(:disabled) {
    background: transparent;
    color: var(--text);
    border-color: transparent;
  }

  main {
    flex: 1;
    overflow-y: auto;
    padding: 28px 28px 40px;
    /* Contenu animé à part lors d'un changement d'onglet */
    view-transition-name: page;
  }

  h1 {
    font-size: var(--fs-5);
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  /* Messages */
  .msg {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
    padding: 8px 8px 8px 16px;
    background: var(--surface);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-sm);
  }
  .msg span {
    flex: 1;
  }
  .note {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    color: var(--warn);
    margin: 0 0 20px;
    font-size: var(--fs-1);
  }
  .note button {
    padding: 4px 12px;
    font-size: var(--fs-1);
  }

  /* Boutons ronds à icône */
  .icon {
    flex: none;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    background: var(--surface-2);
    color: var(--text-soft);
  }
  .icon:hover:not(:disabled) {
    color: var(--text);
  }
  .icon svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  /* Bibliothèque : titre de la console et nombre de jeux */
  .lib-head {
    display: flex;
    align-items: baseline;
    gap: 14px;
    margin-bottom: 20px;
  }
  .count {
    color: var(--text-soft);
    font-size: var(--fs-2);
    font-variant-numeric: tabular-nums;
  }

  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 28px;
  }
  .spacer {
    flex: 1;
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
    flex: 0 1 260px;
    min-width: 140px;
  }
  .search svg {
    position: absolute;
    left: 12px;
    width: 14px;
    height: 14px;
    fill: none;
    stroke: var(--text-soft);
    stroke-width: 1.8;
    stroke-linecap: round;
    pointer-events: none;
  }
  .search input {
    width: 100%;
    padding-left: 34px;
    border-radius: 999px;
  }
  .bar select {
    border-radius: 999px;
  }

  /* État vide : une invitation à agir */
  .empty {
    max-width: 46ch;
    padding: 40px 0;
    color: var(--text-soft);
  }
  .empty p {
    margin: 0 0 6px;
  }
  .empty-title {
    color: var(--text);
    font-weight: 600;
    font-size: var(--fs-3);
  }
  .empty-acts {
    display: flex;
    gap: 8px;
    margin-top: 20px;
  }

  /* Grille de jaquettes */
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--cover, 140px), 1fr));
    gap: 28px 22px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    /* Apparition en cascade à l'ouverture de la vue */
    animation: rise 440ms var(--ease) both;
    animation-delay: calc(var(--i, 0) * 40ms);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(24px) scale(0.94);
    }
  }
  /* Jeu lancé : sa jaquette s'efface, sa copie vole vers le centre */
  .card.launched .box {
    visibility: hidden;
  }
  .ghost {
    position: fixed;
    z-index: 21;
    border-radius: var(--radius-sm);
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--surface);
    color: var(--line);
    font-size: var(--fs-6);
    font-weight: 700;
    pointer-events: none;
    box-shadow:
      var(--ring),
      0 0 48px 10px var(--halo),
      0 18px 40px var(--shadow);
    animation:
      fly-center 700ms var(--ease) forwards,
      glow 1800ms 700ms ease-in-out forwards;
  }
  .ghost img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  @keyframes fly-center {
    from {
      transform: scale(var(--from, 1));
      opacity: calc(var(--from, 1) * 1.6 - 0.6);
    }
    to {
      transform: translate(var(--tx), var(--ty)) scale(1.6);
      opacity: 1;
    }
  }
  /* Pendant que melonDS démarre, le halo s'intensifie */
  @keyframes glow {
    to {
      box-shadow:
        var(--ring),
        0 0 90px 26px var(--halo),
        0 24px 60px var(--shadow);
    }
  }
  .launch-caption {
    position: fixed;
    left: 0;
    right: 0;
    z-index: 21;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    pointer-events: none;
    animation: caption-in 500ms 450ms var(--ease) both;
  }
  .launch-title {
    font-size: var(--fs-4);
    font-weight: 700;
    letter-spacing: -0.01em;
    text-align: center;
    padding: 0 24px;
  }
  .launch-bar {
    width: 220px;
    height: 3px;
    border-radius: 999px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .launch-bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    animation: fill 2500ms linear both;
  }
  @keyframes caption-in {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }
  @keyframes fill {
    from {
      transform: scaleX(0);
    }
  }
  /* Halo qui respire autour de l'élément sélectionné : trois respirations (12 s) puis il se fige,
     pour ne pas faire tourner la carte graphique en continu. Seulement au premier plan. */
  .card::after,
  .tile::after {
    content: "";
    position: absolute;
    z-index: -1;
    border-radius: var(--radius-sm);
    box-shadow: 0 0 36px 10px var(--halo);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--mid) var(--ease);
  }
  .card::after {
    top: 0;
    left: 0;
    right: 0;
    aspect-ratio: 10 / 9;
  }
  .tile::after {
    inset: 0;
    border-radius: var(--radius);
  }
  .card:has(> .box:hover:not(:disabled))::after,
  .card:has(> .box:focus-visible)::after,
  :global(html[data-pad]) .card:has(> .box:focus)::after,
  .tile.on::after {
    opacity: 0.6;
  }
  :global(html[data-awake]) .card:has(> .box:hover:not(:disabled))::after,
  :global(html[data-awake]) .card:has(> .box:focus-visible)::after,
  :global(html[data-awake][data-pad]) .card:has(> .box:focus)::after,
  :global(html[data-awake]) .tile.on::after {
    animation: breathe 4s ease-in-out 3;
  }
  /* Calque propre : la carte graphique anime sans redessiner l'ombre */
  .card::after,
  .tile::after {
    will-change: transform, opacity;
  }
  /* Part de l'état de repos (0,6) et y revient : aucun saut à la fin */
  @keyframes breathe {
    0%,
    100% {
      opacity: 0.6;
      transform: scale(1);
    }
    30% {
      opacity: 1;
      transform: scale(1.06);
    }
    75% {
      opacity: 0.25;
      transform: scale(0.98);
    }
  }

  /* Onde de sélection : un anneau accent part de la jaquette, une seule fois */
  .card::before {
    content: "";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    aspect-ratio: 10 / 9;
    border-radius: var(--radius-sm);
    box-shadow: 0 0 0 2px var(--accent);
    opacity: 0;
    pointer-events: none;
  }
  .card:has(> .box:hover:not(:disabled))::before,
  .card:has(> .box:focus-visible)::before,
  :global(html[data-pad]) .card:has(> .box:focus)::before {
    animation: ripple 650ms var(--ease);
  }
  @keyframes ripple {
    from {
      opacity: 0.9;
      transform: scale(1);
    }
    to {
      opacity: 0;
      transform: scale(1.3);
    }
  }
  /* Effet de boîte en 3D, uniquement au survol */
  .box {
    padding: 0;
    border: none;
    background: var(--surface);
    border-radius: var(--radius-sm);
    overflow: hidden;
    display: block;
    /* Au repos, anneau et lueur sont larges et transparents : à la sélection ils se resserrent */
    box-shadow:
      0 0 0 8px transparent,
      0 0 0 12px transparent,
      0 0 0 0 transparent,
      0 0 0 transparent;
    transition:
      transform var(--mid) var(--ease),
      box-shadow var(--mid) var(--ease);
  }
  .box:hover:not(:disabled) {
    background: var(--surface);
  }
  /* Le halo : anneau accent et lueur autour de la jaquette sélectionnée.
     --x et --y suivent la souris : le côté survolé s'enfonce */
  .box:hover:not(:disabled),
  .box:focus-visible {
    transform: perspective(600px) rotateX(calc(var(--y, 0) * -12deg)) rotateY(calc(var(--x, 0) * 12deg)) scale(1.04);
    box-shadow:
      var(--ring),
      0 0 28px 4px var(--halo),
      calc(var(--x, 0) * -8px) calc(var(--y, 0) * -8px + 8px) 20px var(--shadow);
  }
  .box:disabled {
    opacity: 1;
  }
  .box img {
    display: block;
    width: 100%;
    height: auto;
  }
  /* Jaquette absente : l'initiale du jeu */
  .ph {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 10 / 9;
    font-size: var(--fs-6);
    font-weight: 700;
    line-height: 1;
    color: var(--line);
    background: var(--surface);
  }
  .title {
    font-size: var(--fs-1);
    font-weight: 500;
    /* Jusqu'à deux lignes, puis points de suspension */
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
    line-height: 1.3;
    color: var(--text-soft);
  }
  .card:hover .title,
  .card:focus-within .title {
    color: var(--text);
  }
  .edit {
    font-size: var(--fs-1);
    width: 100%;
    padding: 2px 8px;
    border: 1px solid var(--accent) !important;
  }
  .acts {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 4px;
    visibility: hidden;
  }
  .acts .icon {
    background: var(--chip);
  }
  .card:hover .acts,
  .card:focus-within .acts {
    visibility: visible;
  }

  /* Réglages : titre de section à gauche, lignes à droite */
  .settings {
    max-width: 820px;
  }
  .settings h1 {
    margin-bottom: 12px;
  }
  section {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 8px 32px;
    padding: 22px 0;
    border-top: 1px solid var(--surface-2);
  }
  .settings h1 + section {
    border-top: none;
  }
  h2 {
    font-size: var(--fs-3);
    padding-top: 8px;
  }
  .rows {
    min-width: 0;
  }
  h3 {
    font-size: var(--fs-1);
    font-weight: 600;
    color: var(--text-soft);
    margin: 20px 0 8px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 10px 0;
    border-top: 1px solid var(--surface-2);
  }
  .row:first-child {
    border-top: none;
  }
  .lbl {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .lbl small {
    color: var(--text-soft);
    font-size: var(--fs-1);
    max-width: 52ch;
  }
  .row select {
    flex: none;
    min-width: 220px;
  }
  .row > button {
    flex: none;
  }
  .range {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .range input {
    width: 180px;
  }
  .val {
    min-width: 52px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-soft);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 0 0;
    font-size: var(--fs-1);
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: currentColor;
  }
  .ok {
    color: var(--accent);
  }
  .ko {
    color: var(--danger);
  }
  .hint {
    color: var(--text-soft);
    font-size: var(--fs-1);
    margin: 8px 0 0;
    max-width: 60ch;
  }

  /* Touches */
  .keys {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(270px, 1fr));
    gap: 6px 24px;
  }
  .krow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .klabel {
    flex: 1;
    min-width: 0;
  }
  .kbtn {
    width: 132px;
    text-align: left;
    border-radius: var(--radius-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kbtn.wait {
    border-color: var(--accent);
    color: var(--accent);
  }
  .kreset {
    margin-top: 20px;
  }

  /* Panneau de configuration d'une console */
  .lead {
    color: var(--text-soft);
    font-size: var(--fs-3);
    line-height: 1.5;
    margin: 12px 0 28px;
    max-width: 52ch;
  }
  .found {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
  }
  .found li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--surface-2);
  }
  .found li:first-child {
    border-top: none;
  }
  .found button {
    flex: none;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .link:hover:not(:disabled) {
    background: none;
    border: none;
  }
  .steps {
    list-style: none;
    counter-reset: step;
    margin: 0;
    padding: 0;
  }
  .steps li {
    counter-increment: step;
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding: 4px 0 24px 48px;
  }
  /* Numéro d'étape : de vraies étapes, dans l'ordre */
  .steps li::before {
    content: counter(step);
    position: absolute;
    left: 0;
    top: 0;
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-2);
    font-weight: 700;
    font-size: var(--fs-1);
  }
  .step-acts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .legal {
    margin: 16px 0 0;
    padding-top: 16px;
    border-top: 1px solid var(--surface-2);
    color: var(--text-soft);
    font-size: var(--fs-1);
  }

  /* Choix segmenté (thème) */
  .seg {
    flex: none;
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--surface-2);
    border-radius: 999px;
  }
  .seg button {
    background: transparent;
    color: var(--text-soft);
    padding: 5px 14px;
  }
  .seg button:hover:not(:disabled) {
    background: var(--surface);
    border-color: transparent;
    color: var(--text);
  }
  .seg button.on,
  .seg button.on:hover:not(:disabled) {
    background: var(--bg);
    color: var(--accent);
    font-weight: 600;
  }

  /* Menu des consoles */
  .home {
    max-width: 1100px;
    margin: 0 auto;
    padding-top: 12px;
  }
  .home-head {
    min-height: 120px;
    margin-bottom: 28px;
  }
  .home-title {
    font-size: var(--fs-6);
    letter-spacing: -0.03em;
  }
  .home-sub {
    margin: 8px 0 0;
    color: var(--text-soft);
    font-size: var(--fs-3);
    max-width: 60ch;
  }
  .shelf {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 18px;
  }
  .shelf li {
    animation: rise 460ms var(--ease) both;
    animation-delay: calc(var(--i, 0) * 60ms);
  }
  .tile {
    position: relative;
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 18px 16px 16px;
    border-radius: var(--radius);
    background: var(--surface);
    text-align: left;
    box-shadow:
      0 0 0 8px transparent,
      0 0 0 12px transparent,
      0 0 0 0 transparent;
    transition:
      transform var(--mid) var(--ease),
      box-shadow var(--mid) var(--ease),
      opacity var(--mid) var(--ease);
  }
  .tile:hover:not(:disabled) {
    background: var(--surface);
    border-color: transparent;
  }
  .tile.soon {
    opacity: 0.55;
  }
  /* Console sélectionnée : elle grandit et prend le halo */
  .tile.on {
    opacity: 1;
    transform: translateY(-4px) scale(1.05);
    box-shadow:
      var(--ring),
      0 0 32px 6px var(--halo);
  }
  .tile-art {
    width: 100%;
    height: 72px;
    margin-bottom: 10px;
    fill: none;
    stroke: var(--text-soft);
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition:
      stroke var(--mid) var(--ease),
      transform var(--mid) var(--ease);
  }
  .tile.on .tile-art {
    stroke: var(--accent);
    transform: scale(1.08);
  }
  .tile-name {
    font-weight: 600;
  }
  .tile-state {
    font-size: var(--fs-1);
    color: var(--text-soft);
  }
  .tile.on:not(.soon) .tile-state {
    color: var(--accent);
  }
  /* Console prise en charge mais pas encore prête : petite pastille */
  .tile-state.todo::before {
    content: "";
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--accent);
    vertical-align: middle;
  }
  .setup-head {
    align-items: center;
  }
  .setup-art {
    flex: none;
    width: 72px;
    height: 54px;
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .sheet .lead {
    font-size: var(--fs-2);
    margin: 8px 0 20px;
  }
  .sheet .status {
    margin: 0 0 16px;
  }
  .sheet .seg {
    margin-bottom: 20px;
  }

  /* Groupes par franchise */
  .group-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 32px 0 14px;
  }
  .group-head:first-of-type {
    margin-top: 0;
  }
  .group-head h2 {
    padding: 0;
  }
  .fr-input {
    width: 180px;
    padding: 4px 10px;
  }

  /* Sélecteur de taille des jaquettes */
  .sizes {
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--surface-2);
    border-radius: 999px;
  }
  .size {
    display: grid;
    place-items: center;
    width: 30px;
    height: 28px;
    padding: 0;
    background: transparent;
    color: var(--text-soft);
  }
  .size:hover:not(:disabled) {
    background: var(--surface);
    border-color: transparent;
    color: var(--text);
  }
  .size.on,
  .size.on:hover:not(:disabled) {
    background: var(--bg);
    color: var(--accent);
  }
  .size svg {
    width: 14px;
    height: 14px;
    fill: currentColor;
  }
  .size rect {
    rx: 0.8px;
  }

  /* Voile de lancement */
  .launch-veil {
    position: fixed;
    inset: 0;
    z-index: 20;
    background: var(--veil-strong);
    pointer-events: none;
  }

  /* Favoris */
  .fav-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .fav-toggle svg,
  .fav svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linejoin: round;
  }
  .fav-toggle.on {
    color: var(--accent);
    border-color: var(--accent);
  }
  .fav-toggle.on svg,
  .icon.on svg,
  .fav svg {
    fill: currentColor;
  }
  /* Petit rebond de l'étoile quand elle s'allume */
  .fav-toggle.on svg,
  .icon.on svg {
    animation: pop var(--slow) var(--ease);
  }
  @keyframes pop {
    40% {
      transform: scale(1.7);
    }
  }
  .icon.on {
    color: var(--accent);
  }
  /* Étoile toujours visible sur la jaquette d'un favori */
  .fav {
    position: absolute;
    top: 6px;
    left: 6px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--chip);
    color: var(--accent);
    pointer-events: none;
  }

  /* Bandeau Reprendre : la dernière partie */
  .resume {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 28px;
    padding: 12px 16px 12px 12px;
    background: var(--surface);
    border-radius: var(--radius);
  }
  .resume-cover {
    flex: none;
    width: 64px;
    aspect-ratio: 10 / 9;
    border-radius: 6px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--line);
    font-weight: 700;
    font-size: var(--fs-4);
  }
  .resume-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .resume-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .resume-title {
    font-size: var(--fs-3);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Fiche du jeu : panneau à droite */
  .sheet-back {
    position: fixed;
    inset: 0;
    z-index: 5;
    background: var(--veil);
  }
  .sheet {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 6;
    width: min(440px, 100%);
    overflow-y: auto;
    padding: 24px;
    background: var(--surface);
    border-left: 1px solid var(--surface-2);
  }
  .sheet-head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 8px;
  }
  .sheet-cover {
    flex: none;
    width: 112px;
    aspect-ratio: 10 / 9;
    border-radius: var(--radius-sm);
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: var(--line);
    font-size: var(--fs-5);
    font-weight: 700;
  }
  .sheet-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .sheet-id {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
  .sheet-id h2 {
    padding: 0;
    overflow-wrap: anywhere;
  }
  .sheet-id .hint {
    margin: 0 0 8px;
  }
  .sheet h3 {
    margin-top: 24px;
  }
  .sheet-line {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin: 0 0 12px;
  }
  .soft {
    color: var(--text-soft);
    font-size: var(--fs-1);
  }
  .sheet-acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .sheet-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .sheet-list li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
    padding: 8px 0;
    border-top: 1px solid var(--surface-2);
  }
  .sheet-list li:first-child {
    border-top: none;
  }

  /* Glisser-déposer : calque sur toute la fenêtre */
  .drop {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--veil-strong);
    pointer-events: none;
  }
  .drop-zone {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: var(--radius);
    border: 2px dashed var(--accent);
    box-shadow: 0 0 40px 4px var(--halo) inset;
    color: var(--text-soft);
    text-align: center;
  }
  .drop-zone p {
    margin: 0;
  }
  .drop-title {
    color: var(--text);
    font-size: var(--fs-4);
    font-weight: 700;
  }

  /* Mode manette : l'anneau suit le focus, le halo reste sur les jaquettes */
  :global(html[data-pad] :focus) {
    box-shadow: var(--ring);
  }
  :global(html[data-pad]) .box:focus {
    transform: scale(1.04);
    box-shadow:
      var(--ring),
      0 0 28px 4px var(--halo),
      0 8px 20px var(--shadow);
  }
  :global(html[data-pad]) {
    cursor: none;
  }
  /* Renommer et retirer restent à la souris */
  :global(html[data-pad]) .card .acts {
    visibility: hidden;
  }
  .pad-help {
    flex: none;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 20px;
    padding: 10px 28px;
    border-top: 1px solid var(--surface-2);
    color: var(--text-soft);
    font-size: var(--fs-1);
  }
  .pad-help span {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  kbd {
    display: inline-grid;
    place-items: center;
    min-width: 22px;
    height: 22px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-weight: 600;
    font-size: 11px;
  }

  /* Fenêtre étroite : le titre de section repasse au-dessus */
  @media (max-width: 700px) {
    header,
    main {
      padding-left: 16px;
      padding-right: 16px;
    }
    section {
      grid-template-columns: 1fr;
    }
    h2 {
      padding-top: 0;
    }
  }
</style>
