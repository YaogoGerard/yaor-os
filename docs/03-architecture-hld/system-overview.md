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
