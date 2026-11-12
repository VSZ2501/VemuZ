# VemuZ

Interface unique pour plusieurs émulateurs de consoles, pensée pour rester simple, fluide et très légère (mémoire, processeur, carte graphique). Windows en priorité.

Première console prise en charge : Nintendo DS, via [melonDS].

## Fonctionnalités (V0.1)

- Choix du dossier de melonDS, avec vérification de l'exécutable
- Ajout de jeux `.nds` un par un ou depuis un dossier entier
- Bibliothèque avec titre et code lus dans l'en-tête de la ROM
- Lancement d'un jeu en un clic : VemuZ se réduit pendant la partie et revient à la fermeture de melonDS

## Prérequis

- Windows 10 ou 11
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

## Stack

Tauri 2 (Rust), Svelte, TypeScript, Vite.

## Légal

VemuZ ne fournit et ne télécharge jamais de ROM, de BIOS ou de firmware. Tu installes melonDS toi-même et tu indiques son dossier à VemuZ.