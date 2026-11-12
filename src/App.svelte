<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  type Game = { path: string; title: string; code: string };
  type Settings = { melonds_dir: string | null };

  let view: "library" | "settings" = $state("library");
  let games: Game[] = $state([]);
  let settings: Settings = $state({ melonds_dir: null });
  let melondsOk = $state(false);
  let message = $state("");

  const errText = (e: unknown) => (typeof e === "string" ? e : String(e));

  onMount(async () => {
    settings = await invoke<Settings>("get_settings");
    melondsOk = await invoke<boolean>("melonds_ok");
    games = await invoke<Game[]>("list_games");
    if (!melondsOk) view = "settings";
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
      <ul>
        {#each games as g (g.path)}
          <li>
            <button class="game" onclick={() => launch(g)} disabled={!melondsOk} title={g.path}>
              <span class="title">{g.title}</span>
              <span class="code">{g.code}</span>
            </button>
            <button class="rm" onclick={() => update("remove_game", { path: g.path })} title="Retirer de la liste">×</button>
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
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    gap: 4px;
    margin-bottom: 4px;
  }
  .game {
    flex: 1;
    display: flex;
    justify-content: space-between;
    text-align: left;
  }
  .code {
    opacity: 0.6;
    font-family: monospace;
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
