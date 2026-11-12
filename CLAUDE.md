# VemuZ

Application PC (Windows en priorité) qui sert d'interface unique entre plusieurs émulateurs de consoles.
Objectif : simple, fluide, et très légère en mémoire, processeur et carte graphique.
Première console : Nintendo DS via melonDS.

## Stack
- Tauri 2 (backend Rust)
- Frontend : Svelte + TypeScript + Vite
- Aucune nouvelle dépendance sans me demander d'abord, en expliquant pourquoi.

## Style de réponse
- Français, réponses courtes et directes. Pas de résumé de ce que tu viens de faire, sauf si je le demande.
- Jamais de tiret cadratin ni d'emoji.
- Commentaires dans le code : en français, courts, seulement quand c'est utile pour comprendre.

## Git
- Interdit : n'exécute jamais de commande git (add, commit, push, etc.). Je gère git moi-même.

## Méthode
- Avant de coder une fonctionnalité, propose un plan court et attends mon accord.
- Une seule fonctionnalité à la fois.
- Après chaque modification, vérifie que le projet compile.
- Ne touche pas au README sans me le demander.

## Performance
- Objectif : moins de 100 Mo de mémoire au repos (tous processus confondus).
- Pas de moteur 3D ni de grosse bibliothèque graphique. Effets visuels en CSS.
- Quand un jeu est lancé, VemuZ se met en veille : fenêtre réduite, aucune animation ni tâche de fond.
- Les images (jaquettes) sont mises en cache en local et redimensionnées.

## Architecture
- Un cœur commun : bibliothèque de jeux, réglages, lancement.
- Un connecteur par émulateur, derrière une interface commune, pour pouvoir en ajouter d'autres sans réécrire le cœur.
- Données utilisateur (réglages, liste des jeux) dans le dossier de données de l'application, en JSON.

## Légal
- VemuZ ne fournit et ne télécharge jamais de ROM, de BIOS ou de firmware.
- L'utilisateur installe melonDS lui-même et indique son dossier à VemuZ.
