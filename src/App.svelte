<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { KEY_ROWS, SUGGESTED, qtName, toQt } from "./keys";

  type Game = {
    path: string;
    title: string;
    code: string;
    db_title?: string | null;
    custom_title?: string | null;
    last_played?: number | null;
  };
  type LaunchOptions = { fullscreen: boolean };
  type Settings = { melonds_dir: string | null; options: LaunchOptions };
  type EmuConfig = {
    screen_layout: number;
    screen_swap: boolean;
    integer_scaling: boolean;
    screen_filter: boolean;
    fast_renderer: boolean;
    limit_fps: boolean;
    jit: boolean;
    volume: number;
    language: number;
    direct_boot: boolean;
    pause_on_blur: boolean;
    fast_forward: number;
  };

  let view: "library" | "settings" = $state("library");
  let games: Game[] = $state([]);
  let settings: Settings = $state({ melonds_dir: null, options: { fullscreen: false } });
  let melondsOk = $state(false);
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
  let sort: "title" | "recent" = $state("title");
  const norm = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

  let shown = $derived.by(() => {
    const q = norm(search.trim());
    const list = q ? games.filter((g) => norm(label(g)).includes(q) || norm(g.code).includes(q)) : [...games];
    // La liste arrive déjà triée par titre ; le tri est stable
    if (sort === "recent") list.sort((a, b) => (b.last_played ?? 0) - (a.last_played ?? 0));
    return list;
  });

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
    melondsOk = await invoke<boolean>("melonds_ok");
    games = await invoke<Game[]>("list_games");
    if (!melondsOk) view = "settings";

    const cached = await invoke<Record<string, string>>("list_covers");
    for (const [code, path] of Object.entries(cached)) covers[code] = convertFileSrc(path);
    await listen<{ code: string; path: string }>("cover-ready", (e) => {
      covers[e.payload.code] = convertFileSrc(e.payload.path);
    });
    await listen<string>("covers-error", (e) => (message = e.payload));
    await listen("games-updated", async () => (games = await invoke<Game[]>("list_games")));
  });

  async function chooseMelonDir() {
    const dir = await open({ directory: true, title: "Dossier de melonDS" });
    if (typeof dir !== "string") return;
    try {
      settings = await invoke<Settings>("set_melonds_dir", { dir });
      melondsOk = true;
      message = "Dossier melonDS enregistré.";
    } catch (e) {
      melondsOk = false;
      message = errText(e);
    }
  }

  // Réglages de melonDS, lus et écrits dans son fichier de configuration
  let emu: EmuConfig | null = $state(null);
  let emuError = $state("");

  // Touches du clavier : nom du bouton vers code Qt (-1 : non assignée)
  let keys: Record<string, number> | null = $state(null);
  // Bouton en attente d'une touche
  let capturing: string | null = $state(null);

  async function loadEmu() {
    try {
      emu = await invoke<EmuConfig>("get_emulator_config");
      keys = await invoke<Record<string, number>>("get_keybindings");
      emuError = "";
    } catch (e) {
      emu = null;
      keys = null;
      emuError = errText(e);
    }
  }

  async function saveKeys(next: Record<string, number>) {
    try {
      keys = await invoke<Record<string, number>>("set_keybindings", { keys: next });
    } catch (e) {
      message = errText(e);
      await loadEmu();
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

  function onKeyDown(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    if (e.key === "Escape") {
      capturing = null;
      return;
    }
    const code = toQt(e);
    if (code === null) {
      message = "Cette touche n'est pas prise en charge.";
      return;
    }
    const name = capturing;
    capturing = null;
    assignKey(name, code);
  }

  async function saveEmu() {
    if (!emu) return;
    try {
      emu = await invoke<EmuConfig>("set_emulator_config", { config: { ...emu } });
      message = "";
    } catch (e) {
      message = errText(e);
      await loadEmu();
    }
  }

  // Recharge les réglages à chaque ouverture de la vue Réglages
  $effect(() => {
    if (view === "settings" && melondsOk) loadEmu();
  });

  async function saveOptions() {
    try {
      await invoke("set_launch_options", { options: { ...settings.options } });
    } catch (e) {
      message = errText(e);
    }
  }

  async function openEmulator() {
    message = "";
    try {
      await invoke("open_emulator");
    } catch (e) {
      message = errText(e);
    }
  }

  async function addFiles() {
    const sel = await open({
      multiple: true,
      title: "Ajouter des jeux",
      filters: [{ name: "Jeux Nintendo DS", extensions: ["nds"] }],
    });
    if (!sel) return;
    await update("add_games", { paths: Array.isArray(sel) ? sel : [sel] });
  }

  async function addFolder() {
    const dir = await open({ directory: true, title: "Dossier de jeux" });
    if (typeof dir === "string") await update("add_folder", { dir });
  }

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

  async function launch(g: Game) {
    message = "";
    try {
      await invoke("launch_game", { path: g.path });
      games = await invoke<Game[]>("list_games");
    } catch (e) {
      message = errText(e);
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<main>
  <nav>
    <button class:active={view === "library"} onclick={() => (view = "library")}>Bibliothèque</button>
    <button class:active={view === "settings"} onclick={() => (view = "settings")}>Réglages</button>
  </nav>

  {#if message}<p class="msg">{message}</p>{/if}

  {#if view === "library"}
    <div class="bar">
      <button onclick={addFiles}>Ajouter des jeux</button>
      <button onclick={addFolder}>Ajouter un dossier</button>
      {#if games.length > 0}
        <input class="search" type="search" placeholder="Rechercher" bind:value={search} />
        <select bind:value={sort} aria-label="Tri">
          <option value="title">Titre (A-Z)</option>
          <option value="recent">Dernier lancé</option>
        </select>
      {/if}
    </div>
    {#if games.length === 0}
      <p class="empty">Aucun jeu. Ajoutez des fichiers .nds.</p>
    {:else if shown.length === 0}
      <p class="empty">Aucun résultat.</p>
    {:else}
      <ul class="grid">
        {#each shown as g (g.path)}
          <li class="card">
            <button
              class="box"
              onclick={() => launch(g)}
              onmousemove={tilt}
              onmouseleave={untilt}
              disabled={!melondsOk}
              title="{g.code}&#10;{g.path}"
            >
              {#if covers[g.code]}
                <img src={covers[g.code]} alt={label(g)} loading="lazy" decoding="async" />
              {:else}
                <span class="ph">{label(g)}</span>
              {/if}
            </button>
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
            <button class="act ren" onclick={() => startEdit(g)} title="Renommer">✎</button>
            <button class="act rm" onclick={() => update("remove_game", { path: g.path })} title="Retirer de la liste">×</button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else}
    <h2>melonDS</h2>
    <p>Dossier : <code>{settings.melonds_dir ?? "non défini"}</code></p>
    <p class={melondsOk ? "ok" : "ko"}>{melondsOk ? "Exécutable trouvé." : "Exécutable introuvable."}</p>
    <button onclick={chooseMelonDir}>Choisir le dossier</button>

    <h2>Lancement</h2>
    <label class="opt">
      <input type="checkbox" bind:checked={settings.options.fullscreen} onchange={saveOptions} />
      Plein écran
    </label>
    <button onclick={openEmulator} disabled={!melondsOk}>Ouvrir melonDS pour le configurer</button>

    {#if melondsOk}
      <h2>Réglages de melonDS</h2>
      {#if emuError}
        <p class="ko">{emuError}</p>
      {:else if emu}
        <h3>Écrans</h3>
        <label class="opt">
          Disposition
          <select bind:value={emu.screen_layout} onchange={saveEmu}>
            <option value={0}>Naturelle</option>
            <option value={1}>Verticale (haut et bas)</option>
            <option value={2}>Horizontale (côte à côte)</option>
            <option value={3}>Hybride (un grand écran)</option>
          </select>
        </label>
        <label class="opt"><input type="checkbox" bind:checked={emu.screen_swap} onchange={saveEmu} /> Échanger les deux écrans</label>
        <label class="opt"><input type="checkbox" bind:checked={emu.integer_scaling} onchange={saveEmu} /> Agrandissement sans flou (multiples entiers)</label>
        <label class="opt"><input type="checkbox" bind:checked={emu.screen_filter} onchange={saveEmu} /> Adoucir l'image</label>

        <h3>Performances</h3>
        <label class="opt">
          Rendu 3D
          <select bind:value={emu.fast_renderer} onchange={saveEmu}>
            <option value={false}>Compatible (logiciel)</option>
            <option value={true}>Rapide (OpenGL)</option>
          </select>
        </label>
        <label class="opt"><input type="checkbox" bind:checked={emu.jit} onchange={saveEmu} /> Accélération JIT</label>
        <p class="hint">Rend l'émulation bien plus rapide. À désactiver si un jeu plante.</p>
        <label class="opt"><input type="checkbox" bind:checked={emu.limit_fps} onchange={saveEmu} /> Limiter à la vitesse normale (60 images par seconde)</label>
        <label class="opt">
          Vitesse de l'avance rapide
          <input type="range" min="100" max="1000" step="25" bind:value={emu.fast_forward} onchange={saveEmu} />
          {emu.fast_forward} %
        </label>
        <p class="hint">
          S'applique tant que la touche d'avance rapide est enfoncée. 100 % est la vitesse normale ; si ton ordinateur ne suit
          pas, la vitesse reste limitée par sa puissance.
        </p>
        <label class="opt"><input type="checkbox" bind:checked={emu.pause_on_blur} onchange={saveEmu} /> Mettre en pause quand la fenêtre perd le focus</label>

        <h3>Audio et langue</h3>
        <label class="opt">
          Volume
          <input type="range" min="0" max="100" bind:value={emu.volume} onchange={saveEmu} />
          {emu.volume} %
        </label>
        <label class="opt">
          Langue des jeux
          <select bind:value={emu.language} onchange={saveEmu}>
            <option value={1}>Anglais</option>
            <option value={2}>Français</option>
            <option value={3}>Allemand</option>
            <option value={4}>Italien</option>
            <option value={5}>Espagnol</option>
            <option value={0}>Japonais</option>
          </select>
        </label>
        <p class="hint">Les jeux disponibles en plusieurs langues utilisent celle-ci.</p>

        {#if keys}
          <h2>Touches du clavier</h2>
          <p class="hint">
            Clique sur un bouton puis appuie sur la touche voulue (Échap annule). Pour la manette, utilise « Ouvrir melonDS pour
            le configurer ».
          </p>
          {#each KEY_ROWS as group}
            <h3>{group.title}</h3>
            <div class="keys">
              {#each group.items as it}
                <span class="klabel">{it.label}</span>
                <span class="kctl">
                  <button class="kbtn" class:wait={capturing === it.name} onclick={() => (capturing = it.name)}>
                    {capturing === it.name ? "Appuie sur une touche…" : qtName(keys[it.name] ?? -1)}
                  </button>
                  <button class="kclr" title="Désassigner" onclick={() => assignKey(it.name, -1)} disabled={(keys[it.name] ?? -1) < 0}>×</button>
                </span>
              {/each}
            </div>
          {/each}
          <button class="kreset" onclick={() => saveKeys({ ...SUGGESTED })}>Valeurs conseillées</button>
        {/if}

        <h3>Démarrage</h3>
        <label class="opt"><input type="checkbox" bind:checked={emu.direct_boot} onchange={saveEmu} /> Lancer directement le jeu</label>
        <p class="hint">
          Saute le menu de la console. Décoché, melonDS démarre via le firmware, un fichier de ta propre console que VemuZ ne
          fournit pas.
        </p>
      {/if}
    {/if}
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, sans-serif;
    background: #1b1b1f;
    color: #e6e6e6;
  }
  main {
    padding: 16px;
  }
  nav,
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
  }
  .search,
  select {
    background: #2d2d34;
    color: inherit;
    border: 1px solid #3c3c46;
    border-radius: 4px;
    padding: 6px 10px;
    font: inherit;
  }
  .search {
    flex: 1;
    min-width: 120px;
    max-width: 260px;
  }
  button {
    background: #2d2d34;
    color: inherit;
    border: 1px solid #3c3c46;
    border-radius: 4px;
    padding: 6px 12px;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: #3a3a44;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button.active {
    border-color: #7a8cff;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 16px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  /* Effet de boîte en 3D, uniquement au survol */
  .box {
    padding: 0;
    border: none;
    background: #2d2d34;
    border-radius: 3px;
    overflow: hidden;
    display: block;
    transition: transform 0.15s, box-shadow 0.15s;
  }
  /* --x et --y suivent la souris : le côté survolé s'enfonce */
  .box:hover:not(:disabled) {
    transform: perspective(600px) rotateX(calc(var(--y, 0) * -12deg)) rotateY(calc(var(--x, 0) * 12deg)) scale(1.04);
    box-shadow: calc(var(--x, 0) * -8px) calc(var(--y, 0) * -8px + 4px) 14px rgba(0, 0, 0, 0.5);
  }
  .box img {
    display: block;
    width: 100%;
    height: auto;
  }
  .ph {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 10 / 9;
    padding: 8px;
    box-sizing: border-box;
    font-size: 0.8rem;
    text-align: center;
    background: linear-gradient(135deg, #34343e, #25252c);
  }
  .title {
    font-size: 0.8rem;
    /* Jusqu'à deux lignes, puis points de suspension */
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
    line-height: 1.25;
  }
  .edit {
    font-size: 0.8rem;
    width: 100%;
    box-sizing: border-box;
    background: #2d2d34;
    color: inherit;
    border: 1px solid #7a8cff;
    border-radius: 3px;
    padding: 0 4px;
  }
  .act {
    position: absolute;
    top: 2px;
    padding: 0 6px;
    display: none;
  }
  .rm {
    right: 2px;
  }
  .ren {
    right: 28px;
  }
  .card:hover .act {
    display: block;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .keys {
    display: grid;
    grid-template-columns: max-content max-content;
    gap: 6px 16px;
    align-items: center;
  }
  .kctl {
    display: flex;
    gap: 4px;
  }
  .kbtn {
    min-width: 170px;
    text-align: left;
  }
  .kbtn.wait {
    border-color: #ffd479;
    color: #ffd479;
  }
  .kclr {
    padding: 0 8px;
  }
  .kreset {
    margin-top: 12px;
  }
  h3 {
    margin: 16px 0 8px;
    font-size: 1rem;
    opacity: 0.85;
  }
  .hint {
    opacity: 0.6;
    font-size: 0.85rem;
    margin: 0 0 8px;
  }
  .msg {
    color: #ffd479;
  }
  .empty {
    opacity: 0.7;
  }
  .ok {
    color: #7fd48a;
  }
  .ko {
    color: #ff8a8a;
  }
</style>
