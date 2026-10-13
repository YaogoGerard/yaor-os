# Software Architecture Document — YAOR

## Couches

```text
Applications
     |
YAOR Desktop
     |
YAOR Compositor
     |
Graphics / Input / Audio
     |
System Services
     |
Userland
     |
Linux Kernel
     |
Hardware
```

## Responsabilités

### Linux Kernel
Gestion bas niveau du matériel, processus, mémoire, réseau et primitives système.

### Existing Components
Composants open source réutilisés lorsque leur fonction répond au besoin.

### YAOR Components
Composants développés pour le Desktop Core, la configuration, les profils Caméléon, le plugin system et l'accessibilité.

Les frontières entre composants doivent être explicites afin de faciliter leur remplacement.
