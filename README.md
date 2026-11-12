# VemuZ

Interface unique pour plusieurs émulateurs de consoles, pensée pour rester simple, fluide et très légère (mémoire, processeur, carte graphique). Windows en priorité.

Première console prise en charge : Nintendo DS, via [melonDS].

## Fonctionnalités

- Choix du dossier de melonDS, avec vérification de l'exécutable
- Ajout de jeux `.nds` un par un ou depuis un dossier entier
- Bibliothèque en grille de jaquettes, triée par titre
- Titres complets issus de la base GameTDB, avec le titre de l'en-tête de la ROM en secours
- Renommage d'un jeu (un titre vide rétablit l'original)
- Jaquettes téléchargées automatiquement depuis GameTDB, puis mises en cache : l'application fonctionne ensuite hors ligne
- Lancement d'un jeu en un clic : VemuZ se réduit pendant la partie et revient à la fermeture de melonDS

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

Les seuls accès réseau sont les téléchargements depuis [GameTDB](https://www.gametdb.com) : la base de titres (rafraîchie tous les 30 jours) et les jaquettes. Rien n'est téléchargé pendant qu'un jeu est lancé.

## Stack

Tauri 2 (Rust), Svelte, TypeScript, Vite.

## Légal

VemuZ ne fournit et ne télécharge jamais de ROM, de BIOS ou de firmware. Tu installes melonDS toi-même et tu indiques son dossier à VemuZ. Les jaquettes et les titres appartiennent à leurs ayants droit et proviennent de GameTDB.