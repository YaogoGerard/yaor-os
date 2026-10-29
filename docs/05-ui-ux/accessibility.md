# Accessibility UX

**Statut :** planifié — *mise à jour : 29 octobre 2026*

Fonctions prévues : lecture d'informations accessibles, lecture au
survol lorsque configurée, sortie vocale, sélection de langue,
vitesse de lecture et activation/désactivation par raccourci.

Liens exigences : REQ-SW-012 (architecture d'accessibilité
indépendante des profils de bureau), REQ-SW-013 (sortie vocale en
Mooré lorsque les composants nécessaires sont disponibles).

Principes :

- **Indépendance des profils** : l'accessibilité est une couche du
  Desktop Core — elle fonctionne identiquement sur les profils
  Classique, macOS-like et Tiling ([`desktop-modes.md`](desktop-modes.md)) ;
- **Activation par raccourci** : tout comme les raccourcis de
  fenêtrage (`keys.*`), les fonctions d'accessibilité sont
  configurables dans la configuration déclarative (REQ-SW-015) ;
- **Contraste** : l'identité « ambre sur noir » offre un contraste
  élevé ; le thème « High Contrast » (voir [`themes/`](themes/))
  l'accentuera ;
- **Sortie vocale** : chemin audio déjà posé (ALSA/`pw-cat` QA) ;
  les voix et la lecture restent des composants remplaçables
  (REQ-SW-014).

Vérification : [`../06-testing/accessibility-tests.md`](../06-testing/accessibility-tests.md).
