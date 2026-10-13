# Desktop Architecture

```mermaid
flowchart TB
    CFG[Configuration] --> CORE[Desktop Core]
    PROFILE[Profiles] --> CORE
    CORE --> WM[Window Manager]
    CORE --> UI[UI Engine]
    CORE --> PLUG[Plugin System]
    WM --> COMP[YAOR Compositor]
    UI --> COMP
```

Le Desktop Core coordonne fenêtres, espaces de travail, panneaux, dock, launcher, thèmes, plugins, profils et configuration.

Les profils sont des configurations de l'expérience de bureau, pas trois systèmes indépendants.

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
