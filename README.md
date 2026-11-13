# VemuZ

Interface unique pour plusieurs émulateurs de consoles, pensée pour rester simple, fluide et très légère (mémoire, processeur, carte graphique). Windows en priorité.

Première console prise en charge : Nintendo DS, via melonDS.

## Fonctionnalités

- Choix du dossier de melonDS, avec vérification de l'exécutable
- Ajout de jeux `.nds` un par un ou depuis un dossier entier
- Bibliothèque en grille de jaquettes, avec recherche (sans accents ni majuscules) et tri par titre ou par dernier lancé
- Titres complets issus de la base GameTDB, avec le titre de l'en-tête de la ROM en secours, et renommage possible
- Jaquettes téléchargées automatiquement depuis GameTDB, puis mises en cache : l'application fonctionne ensuite hors ligne
- Lancement d'un jeu en un clic : VemuZ se réduit pendant la partie et revient à la fermeture de melonDS
- Réglages simplifiés de melonDS depuis VemuZ :
  - écrans (disposition, échange, agrandissement, adoucissement)
  - rendu 3D, accélération JIT, limite de vitesse, pause à la perte du focus
  - volume, langue des jeux, démarrage direct
  - vitesse de l'avance rapide en pourcentage
  - touches du clavier et raccourcis
- Plein écran au lancement et ouverture de melonDS seul pour les réglages avancés (manette comprise)

## Prérequis

- Windows 10 ou 11 (`curl.exe`, livré avec Windows, sert aux téléchargements)
- melonDS installé séparément
- Pour développer : Node.js, Rust et les prérequis de [Tauri 2](https://tauri.app/start/prerequisites/)

## Développement

```bash
npm install
npm run tauri dev
```

Build de production :

```bash
npm run tauri build
```

## Données et réseau

Les réglages, la liste des jeux, la base de titres et les jaquettes sont stockés dans le dossier de données de l'application (`%APPDATA%\com.vemuz.app`).

Les réglages de l'émulateur sont écrits directement dans le fichier `melonDS.toml` de melonDS. Une copie de sécurité `melonDS.toml.bak` est créée avant la première modification, et rien n'est écrit pendant que melonDS tourne.

Les seuls accès réseau sont les téléchargements depuis [GameTDB](https://www.gametdb.com) : la base de titres (rafraîchie tous les 30 jours) et les jaquettes. Rien n'est téléchargé pendant qu'un jeu est lancé.

## Limites connues

- Le son peut grésiller en avance rapide : ce comportement vient de melonDS.
- Les touches de la manette se configurent dans melonDS (bouton « Ouvrir melonDS » dans les Réglages).

## Stack

Tauri 2 (Rust), Svelte, TypeScript, Vite.

## Légal

VemuZ ne fournit et ne télécharge jamais de ROM, de BIOS ou de firmware. Tu installes melonDS toi-même et tu indiques son dossier à VemuZ. Les jaquettes et les titres appartiennent à leurs ayants droit et proviennent de GameTDB.