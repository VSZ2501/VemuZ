# VemuZ : état d'avancement

Document de reprise pour une nouvelle conversation. À lire avec `CLAUDE.md` (règles du projet). Ne pas modifier `README.md` sans demande, ne jamais exécuter de commande git.

## État

- Version courante : **V0.3** (terminée, testée à la main, commitée et taguée par l'utilisateur).
- Historique : V0.1 (réglages, ajout de jeux, bibliothèque, lancement), V0.2 (jaquettes, titres complets, renommage, effet 3D au survol), V0.3 (recherche et tri, réglages de melonDS, touches, avance rapide).
- Mémoire en build release (`npm run tauri build -- --no-bundle`) : environ 96 Mo au repos, environ 104,5 Mo au second relevé. Objectif : moins de 120 Mo. Le processus VemuZ pèse environ 4 Mo, le reste est WebView2 (minimum d'environ 90 Mo, incompressible).

## Stack et dépendances

- Tauri 2 (Rust), Svelte 5, TypeScript, Vite seul (pas de SvelteKit). Identifiant `com.vemuz.app`, crate `vemuz`, lib `vemuz_lib`.
- Seule dépendance ajoutée par rapport au modèle : `tauri-plugin-dialog` (Rust) et `@tauri-apps/plugin-dialog` (JS). Le plugin opener a été retiré.
- Feature Tauri activée : `protocol-asset` (affichage des jaquettes locales).
- Aucune autre dépendance sans accord préalable de l'utilisateur.

## Architecture

Backend, dans `src-tauri/src/` :

- `lib.rs` : état de l'application (réglages, liste des jeux, drapeaux `running` et `fetching`), commandes Tauri, lancement.
- `rom.rs` : lecture de l'en-tête NDS (titre octets 0x00 à 0x0B, code octets 0x0C à 0x0F), structure `Game` (`title`, `db_title`, `custom_title`, `last_played`), `display_title()`.
- `storage.rs` : JSON dans le dossier de données de l'app (`%APPDATA%\com.vemuz.app`), écriture via fichier temporaire.
- `emulator.rs` : trait `Emulator` (`executable`, `launch`), implémentation `MelonDs`, `LaunchOptions` (plein écran, argument `-f`).
- `covers.rs` : worker unique en arrière-plan, un jeu à la fois, téléchargement des jaquettes GameTDB avec `curl.exe`, cache local, marqueurs `.none` (nouvelle tentative après 7 jours).
- `titles.rs` : télécharge `dstdb.txt` (GameTDB), rafraîchi tous les 30 jours, lu ligne par ligne pour renseigner `db_title`.
- `melonds_config.rs` : lecture et écriture ciblée de `melonDS.toml` par un éditeur de lignes (sans dépendance). Réglages simplifiés, touches du clavier. Tests unitaires (`cargo test --lib`).

Frontend, dans `src/` :

- `App.svelte` : vues Bibliothèque et Réglages, grille de jaquettes, recherche, tri, renommage, réglages de melonDS, capture des touches.
- `keys.ts` : conversion des touches du navigateur vers les codes Qt de melonDS, noms lisibles, disposition conseillée, liste des boutons et raccourcis.

Commandes Tauri : `get_settings`, `set_melonds_dir`, `set_launch_options`, `get_emulator_config`, `set_emulator_config`, `get_keybindings`, `set_keybindings`, `open_emulator`, `melonds_ok`, `list_games`, `list_covers`, `add_games`, `add_folder`, `remove_game`, `rename_game`, `launch_game`.

Événements vers le frontend : `cover-ready`, `covers-error`, `games-updated`.

Données utilisateur : `settings.json`, `games.json`, `dstdb.txt`, `covers/<CODE>.jpg` dans `%APPDATA%\com.vemuz.app`.

## Comportements à connaître

- Lancement : melonDS démarre, la fenêtre VemuZ est minimisée, un thread attend la fin du processus puis restaure et refocalise la fenêtre. Pas de téléchargement pendant qu'un jeu tourne (le worker reprend à la fermeture).
- Titre affiché : titre personnalisé, sinon titre GameTDB, sinon titre de l'en-tête de la ROM. Tri par titre ou par dernier lancé (date enregistrée à chaque lancement).
- Réglages de melonDS : écrits dans `melonDS.toml` (à côté de l'exécutable, sinon `%APPDATA%\melonDS`). Copie `melonDS.toml.bak` avant la première modification. Écriture refusée pendant que melonDS tourne (il réécrit son fichier en se fermant).
- Réglages exposés : disposition, échange, agrandissement entier et adoucissement des écrans, rendu 3D (logiciel ou OpenGL, qui règle aussi `Screen.UseGL`), JIT, limite de vitesse, pause à la perte du focus, volume (0 à 100 vers 0 à 256), langue des jeux, démarrage direct, vitesse d'avance rapide (100 à 1000 %, convertie en `FastForwardFPS` selon `TargetFPS`).
- Touches : 12 boutons et 5 raccourcis (plein écran, pause, avance rapide, redémarrer, échanger les écrans), échange automatique en cas de conflit, valeurs conseillées. Section manette de `melonDS.toml` non touchée.

## Pièges et décisions

- PowerShell 5.1 : `Set-Content -Encoding utf8` ajoute un BOM qui casse `tauri.conf.json`. Utiliser les outils d'édition de fichiers.
- Smart App Control bloquait les build scripts de cargo (erreur 4551). Désactivé sur la machine de l'utilisateur, non réactivable sans réinstaller Windows.
- `src-tauri/target` pèse environ 5 Go : normal, ignoré par git, régénérable (`cargo clean`).
- Scope du protocole asset : `$APPDATA/covers/**/*`. En Tauri 2, `$APPDATA` désigne déjà le dossier propre à l'app, pas le dossier Roaming.
- Téléchargements via `curl.exe` (livré avec Windows) plutôt qu'une crate HTTP, pour ne rien ajouter. Fonction `fetch` isolée dans `covers.rs`, remplaçable par `ureq` plus tard si besoin.
- Argument melonDS `-b` : il décide si le jeu démarre (`never` ouvre melonDS vide), il ne choisit pas direct ou firmware. Le mode direct est le réglage `DirectBoot` du fichier de configuration. Seul `-f` (plein écran) est utilisé.
- Son : le grésillement en avance rapide vient de melonDS. Les réglages de lissage et de précision n'apportaient rien et ont été retirés. Le son ne peut pas être dissocié de la vitesse de l'image.
- GameTDB peut ne pas répondre depuis certains réseaux : les erreurs réseau n'écrivent aucun marqueur, le prochain lancement réessaie.

## Tests

- `cargo test --lib` (dans `src-tauri`) : éditeur de `melonDS.toml` (valeurs, sections absentes, fins de ligne CRLF, copie de sécurité, touches).
- `npm run check` et `npm run build` : frontend.
- Le lancement réel avec melonDS, les jaquettes, les titres, la recherche, les réglages et les touches ont été validés à la main par l'utilisateur.

## Pistes pour la suite (aucune n'est décidée)

1. **Veille réelle pendant un jeu** : suspendre la webview (API WebView2) pour libérer de la mémoire. Demande la crate `webview2-com` en dépendance directe (déjà compilée par Tauri), à valider avec l'utilisateur, et une mesure avant et après en release.
2. **Manette** : configuration des boutons. melonDS stocke les numéros SDL, différents de ceux du navigateur ; il faudrait lire la manette côté Rust (crate `gilrs`, à valider). En attendant, bouton « Ouvrir melonDS ».
3. **Second émulateur** : le trait `Emulator` est prêt, `LaunchOptions` à généraliser.
4. **Réglages par jeu** (langue, rendu, etc.).
5. **Refonte de l'interface** (direction artistique, logo, animations, UX) : repoussée à bien plus tard par l'utilisateur. Contraintes : CSS uniquement, aucune animation permanente, aucune animation pendant un jeu, mesure de la mémoire à la fin.

## Méthode de travail convenue

- Réponses en français, courtes, sans tiret cadratin ni emoji.
- Plan court avant chaque fonctionnalité, accord de l'utilisateur, une fonctionnalité à la fois (l'utilisateur peut en regrouper deux, traitées l'une après l'autre).
- Après chaque modification, vérifier que le projet compile.
- Demander avant d'ajouter une dépendance, en expliquant pourquoi.
- Ne jamais exécuter de commande git : l'utilisateur gère le dépôt.
- Ne pas fournir ni télécharger de ROM, de BIOS ou de firmware.
