<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  type Game = {
    path: string;
    title: string;
    code: string;
    db_title?: string | null;
    custom_title?: string | null;
  };
  type Settings = { melonds_dir: string | null };

  let view: "library" | "settings" = $state("library");
  let games: Game[] = $state([]);
  let settings: Settings = $state({ melonds_dir: null });
  let melondsOk = $state(false);
  let message = $state("");
  // Jaquettes en cache : code du jeu vers URL affichable
  let covers: Record<string, string> = $state({});

  // Renommage en cours : chemin du jeu et texte saisi
  let editing: string | null = $state(null);
  let draft = $state("");

  const errText = (e: unknown) => (typeof e === "string" ? e : String(e));
  const label = (g: Game) => g.custom_title ?? g.db_title ?? g.title;

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
    } catch (e) {
      message = errText(e);
    }
  }
</script>

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
    </div>
    {#if games.length === 0}
      <p class="empty">Aucun jeu. Ajoutez des fichiers .nds.</p>
    {:else}
      <ul class="grid">
        {#each games as g (g.path)}
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
    gap: 8px;
    margin-bottom: 12px;
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
