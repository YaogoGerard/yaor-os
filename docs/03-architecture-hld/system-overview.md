# System Overview — YAOR OS

## C4 — System Context

```mermaid
flowchart TB
    U[Utilisateur] --> Y[YAOR OS]
    Y --> A[Applications Linux]
    Y --> H[Hardware via Linux]
```

## C4 — Containers

```mermaid
flowchart TB
    A[Applications] --> D[YAOR Desktop]
    D --> C[YAOR Compositor]
    C --> G[Graphics Stack]
    G --> S[System Services]
    S --> U[Userland]
    U --> K[Linux Kernel]
    K --> H[Hardware]
```

## Principes
- Linux reste la fondation.
- Les composants existants sont réutilisés.
- Rust est privilégié lorsqu'une solution appropriée existe.
- YAOR développe les composants qui définissent son identité.

## Diagramme système as-built (v0.6 étape 1)

Composants réels de l'image live `yaor-live-x86_64-0.2.0.iso` et leurs
liaisons effectives (vérifiés en QA — `graphics-tests.md`). Les liens
en pointillés sont les livraisons à venir du jalon v0.6 « Caméléon ».

```mermaid
flowchart LR
    subgraph BOOT["Chaîne de boot"]
        LIM["Limine (ADR-023)"] --> KER["Noyau Linux custom"]
        KER --> INIT["initramfs live (EROFS + overlay)"]
        INIT --> SD["systemd"]
    end

    subgraph SVC["Services système"]
        UDEV["udev"]
        SEAT["seatd"]
        LOG["systemd-logind"]
        DBUS["D-Bus"]
        IWD["iwd (Wi-Fi)"]
        UDISK["udisks2"]
        PW["PipeWire + wireplumber"]
        USER["user@ (session 0)"]
    end

    subgraph DESK["Session bureau YAOR"]
        COMP["yaor-compositor\n(Smithay 0.7 vendu,\n gabarit anvil)"]
        XWAY["Xwayland"]
        PANEL["yaor-panel\n(layer-shell Top + Overlay)"]
        WT["waytest (C, xdg-shell)"]
        XWT["xwtest (C, client X11)"]
        DOCK["yaor-dock\n(layer-shell Top, centré)"]
    end

    subgraph OUT["Outils produit"]
        INST["yaor-install (mode installé)"]
        SLOT["yaor-slot (A/B + rollback)"]
        CFG["yaor-config"]
    end

    MESA["mesa (EGL/GLES, llvmpipe)"]
    HW["Matériel / QEMU virtio-gpu"]

    %% --- Session bureau : liaisons protocole ---
    PANEL -->|"zwlr_layer_shell_v1 + wl_shm"| COMP
    WT -->|"xdg-shell + wl_shm"| COMP
    XWAY <-->|"X11 ↔ Wayland"| COMP
    XWT -->|"X11 (socket X0)"| XWAY
    PANEL -.->|"lance (env hérité WAYLAND_DISPLAY + DISPLAY=:0)"| WT
    PANEL -.->|"lance (env hérité)"| XWT
    DOCK -->|"zwlr_layer_shell_v1 (Top) + wl_shm + foreign-toplevel"| COMP

    %% --- Compositor : bas de pile ---
    COMP -->|"libseat (session)"| SEAT
    COMP -->|"libinput"| UDEV
    COMP -->|"EGL/GLES"| MESA
    MESA -->|"DRM / KMS (maître)"| HW
    UDEV -->|"evdev (clavier, souris)"| HW
    COMP -->|"session + seat"| LOG
    USER -->|"WAYLAND_DISPLAY, XDG_RUNTIME_DIR"| PANEL

    %% --- Services divers ---
    PW -->|"ALSA"| HW
    IWD -->|"netlink Wi-Fi"| HW
    SD --> UDEV
    SD --> SEAT
    SD --> LOG
    SD --> DBUS
    SD --> IWD
    SD --> UDISK
    SD --> PW
    SD --> USER

    %% --- Outils produit ---
    INST -->|"installe slot B"| SLOT
    CFG -->|"desktop.toml (layout, profil)"| COMP

    %% --- Styles ---
    style BOOT fill:#101014,stroke:#B96A1F,color:#F2E8DC
    style SVC fill:#101014,stroke:#B96A1F,color:#F2E8DC
    style DESK fill:#16161C,stroke:#E8862E,color:#F2E8DC
    style OUT fill:#101014,stroke:#B96A1F,color:#F2E8DC
```

Principes de lecture :

- **un seul Desktop Core à venir** : `yaor-config` (+ profil) pilote
  la politique de fenêtrage du compositor — les profils sont des
  configurations, pas trois systèmes (`desktop-architecture.md`) ;
- le compositor est **maître DRM/KMS** (session via libseat/seatd) —
  les couches layer-shell (Top, Overlay) sont rendues au-dessus de
  l'espace de fenêtrage, leurs zones exclusives soustraites de la zone
  utile du tiling ;
- `yaor-panel` est le point d'entrée des apps YAOR internes (menu) :
  il les lance avec l'environnement hérité (Wayland ET X11) ;
- `yaor-dock` (profil macOS-like, étape 2) est un second client
  layer-shell : ancré **bas seul**, donc centré par le layer-map du
  compositor, **sans zone exclusive** — les fenêtres gardent tout
  l'écran, contrairement à la taskbar du profil classique (zone
  exclusive de 24 px). Il ne dessine que du rendu wl_shm et lit la
  liste des fenêtres par `zwlr_foreign_toplevel_management_v1`, le
  même canal que la taskbar ;
- **le profil (`[profile] name`, étape 3) est relu au SIGHUP par le
  compositor ET par les clients** : pas de protocole privé ni de
  gestionnaire de session — chacun relit `desktop.toml` et applique sa
  part (disposition au compositor ; taskbar au panel, dock au
  `yaor-dock`), le masquage d'une surface se faisant zone exclusive 0
  + région d'entrée vide, sans démonter la couche.
