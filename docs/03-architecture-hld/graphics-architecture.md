# Graphics Architecture

```text
YAOR Desktop
      |
YAOR Compositor
      |
Rust graphics/compositor libraries
      |
Wayland / graphics protocols
      |
Mesa / GPU stack
      |
Linux Kernel
      |
GPU
```

Le compositor YAOR est développé principalement en Rust mais s'appuie sur les primitives existantes appropriées.

Smithay est une base candidate pour éviter de réimplémenter les primitives Wayland et compositor.

La gestion des périphériques d'entrée et de l'audio doit également privilégier une solution Rust adaptée lorsqu'elle existe et est suffisamment mature ; sinon une solution existante appropriée est utilisée.
