# Configuration Engine — LLD

## Objectif
Permettre de modifier le comportement du bureau sans modifier le code.

## Format initial
TOML est le format candidat.

## Arborescence
```text
~/.config/yaor/
├── desktop.toml
├── theme.toml
├── windows.toml
├── keybindings.toml
├── panel.toml
├── launcher.toml
└── accessibility.toml
```

Le moteur doit gérer validation, valeurs par défaut, compatibilité, erreurs et rechargement lorsque possible.
