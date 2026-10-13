# Accessibility Architecture

```text
Applications
     |
Accessibility information
     |
YAOR Accessibility Engine
     |
Language Engine
     |
 +---------+---------+
 |                   |
Translation          TTS
 |                   |
 +---------+---------+
           |
       Mooré voice
```

L'accessibilité est indépendante du profil de bureau.

Le système pourra utiliser les informations d'accessibilité exposées par les applications. L'OCR pourra être étudié comme stratégie de secours lorsque ces informations ne sont pas disponibles.

Le Mooré est une cible prioritaire ; d'autres langues pourront être ajoutées sans modifier le cœur du Desktop Core.
