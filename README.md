# VemuZ

Interface unique pour plusieurs émulateurs de consoles, pensée pour rester simple, fluide et très légère (mémoire, processeur, carte graphique). Windows en priorité.

Première console prise en charge : Nintendo DS, via melonDS.

## Fonctionnalités (V1)

### Consoles et premier lancement

- Menu des consoles au démarrage : seule la Nintendo DS est jouable pour l'instant
- Premier lancement guidé : détection automatique de melonDS dans les dossiers habituels, sinon étapes pour le télécharger et indiquer son dossier

### Bibliothèque

- Grille de jaquettes téléchargées depuis GameTDB, puis mises en cache : l'application fonctionne ensuite hors ligne
- Titres complets issus de la base GameTDB, avec le titre de l'en-tête de la ROM en secours, et renommage possible
- Recherche (sans accents ni majuscules)
- Tri par titre, dernier lancé, temps de jeu ou franchise (groupes reconnus automatiquement, corrigeables)
- Filtre Favoris et trois tailles de jaquettes (sélecteur ou Ctrl + molette)
- Bandeau Reprendre pour relancer le dernier jeu

### Ajout de jeux

- Fichiers `.nds` un par un, ou dossiers entiers
- Glisser-déposer dans la fenêtre
- Archives `.zip` et `.7z` (une archive = un jeu, ouverte directement par melonDS)

### Fiche du jeu

- Sauvegardes et sauvegardes rapides du jeu
- Copies automatiques de la sauvegarde (10 par jeu) et restauration
- Export d'un jeu ou de toutes les sauvegardes en `.zip`
- Temps de jeu, dernière partie, franchise, favori

### Réglages de melonDS depuis VemuZ

- Écrans (disposition, échange, agrandissement, adoucissement)
- Rendu 3D, accélération JIT, limite de vitesse, pause à la perte du focus
- Volume, langue des jeux, démarrage direct
- Vitesse de l'avance rapide en pourcentage
- Touches du clavier et raccourcis
- Configuration de la manette pour melonDS, avec directions à la croix, au stick ou les deux

### Interface

- Navigation complète à la manette dans VemuZ (A valider, B retour, LB et RB pour les onglets, Y pour la recherche)
- Thème sombre, clair ou comme Windows
- Animations légères, coupées si Windows demande moins d'animations
- Lancement animé : VemuZ se réduit pendant la partie et revient à la fermeture de melonDS

## Prérequis

- Windows 10 ou 11 (`curl.exe` et `tar.exe`, livrés avec Windows, servent aux téléchargements et aux archives)
- melonDS installé séparément, par tes soins
- Pour développer : Node.js, Rust et les prérequis de [Tauri 2](https://tauri.app/start/prerequisites/)

## Développement

```bash
npm install
npm run tauri dev
```

Build de production (installateur dans `src-tauri/target/release/bundle/`) :

```bash
npm run tauri build
```

Tests du backend :

```bash
cd src-tauri
cargo test --lib
```

## Données et réseau

Les réglages, la liste des jeux, la base de titres, les jaquettes et les copies de sauvegardes sont stockés dans le dossier de données de l'application (`%APPDATA%\com.vemuz.app`).

Les réglages de l'émulateur sont écrits directement dans le fichier `melonDS.toml` de melonDS. Une copie de sécurité `melonDS.toml.bak` est créée avant la première modification, et rien n'est écrit pendant que melonDS tourne.

Les seuls accès réseau sont les téléchargements depuis [GameTDB](https://www.gametdb.com) : la base de titres (rafraîchie tous les 30 jours) et les jaquettes. Rien n'est téléchargé pendant qu'un jeu est lancé.

## Performance

- Environ 115 à 140 Mo de mémoire au repos, tous processus WebView2 compris (objectif : moins de 150 Mo)
- Processeur quasi nul au repos
- Pas de moteur 3D : effets visuels en CSS

## Limites connues

- Le son peut grésiller en avance rapide : ce comportement vient de melonDS.
- Les archives `.rar` ne sont pas prises en charge.

## Stack

Tauri 2 (Rust), Svelte 5, TypeScript, Vite.

## Légal

VemuZ ne fournit et ne télécharge jamais de ROM, de BIOS ou de firmware. Tu installes melonDS toi-même et tu indiques son dossier à VemuZ. Les jaquettes et les titres appartiennent à leurs ayants droit et proviennent de GameTDB.
