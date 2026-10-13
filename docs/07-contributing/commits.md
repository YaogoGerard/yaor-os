# Commit Convention

**Statut :** Accepté
**Version :** 0.1
**Dernière mise à jour :** 2026-10-13
**Décisions liées :** ADR-008

## 1. Objectif

Chaque commit doit permettre de répondre immédiatement à trois questions :

1. **Sur quelle étape travaille-t-on ?** → trailer `Jalon:`
2. **Sur quelle fonctionnalité travaille-t-on ?** → trailer `Feature:`
3. **Quel est l'objet exact du commit ?** → sujet + corps

Un commit doit être **atomique** : une seule intention par commit.

## 2. Format

```text
<type>(<zone>): <sujet à l'impératif, ≤ 72 caractères>

<Corps : POURQUOI, puis QUOI, puis IMPACT. Obligatoire dès que le commit
n'est pas trivial. Une idée par ligne, ≤ 100 caractères par ligne.>

Jalon: <0.1 | 0.2 | ... | hors-jalon>
Feature: <slug>
Refs: <ADR-xxx, REQ-xxx, TEST-xxx>        # si applicable
```

- Langue : **français** pour le sujet et le corps, **anglais** pour les types, zones
  et noms de trailers.
- Le sujet commence par un **verbe à l'impératif présent** (`ajoute`, `corrige`,
  `documente`…), sans point final et sans majuscule initiale.
- Le code (noms de fichiers, symboles, options) s'écrit en anglais.

## 3. Types (`type`)

| Type | Usage |
|---|---|
| `feat` | Nouvelle fonctionnalité |
| `fix` | Correction de bug |
| `build` | Système de build, toolchain, pipeline, image |
| `docs` | Documentation |
| `test` | Tests |
| `ci` | Intégration continue |
| `refactor` | Refonte sans changement de comportement |
| `perf` | Performance |
| `security` | Sécurité, intégrité, approvisionnement |
| `chore` | Tâches annexes (outillage, nettoyage) |
| `revert` | Annulation d'un commit précédent |

## 4. Zones (`zone`)

Une zone décrit le composant visé. Zones courantes :

`product`, `req`, `arch`, `lld`, `ui`, `test`, `docs`, `adr`,
`build`, `kernel`, `toolchain`, `userland`, `init`, `system-services`,
`graphics`, `compositor`, `desktop`, `config`, `packaging`,
`a11y`, `i18n`, `plugins`, `boot`.

La liste n'est pas fermée : une nouvelle zone doit rester courte et explicite.

## 5. Trailers obligatoires

| Trailer | Obligatoire | Contenu |
|---|---|---|
| `Jalon:` | Oui | Étape en cours (`0.1`, `0.2`…) ou `hors-jalon` |
| `Feature:` | Oui | Slug de la fonctionnalité (ex. `pipeline-sources`, `compositor-wayland`) |
| `Refs:` | Si applicable | Identifiants de la documentation concernée (`ADR-`, `REQ-`, `TEST-`) |

Trailers supplémentaires acceptés : `Co-authored-by:`, `Signed-off-by:`.

## 6. Rupture de compatibilité

- Ajouter `!` après la zone : `feat(config)!: ...`
- Ajouter un trailer `BREAKING CHANGE:` expliquant ce qui casse et la migration.

## 7. Exemples

### Bon

```text
feat(build): ajoute le téléchargement vérifié des sources

Télécharge les sources listées dans manifest.tsv et vérifie chaque empreinte
SHA-256 avant de poursuivre. Échoue explicitement en cas de mismatch pour
éviter un build sur des sources altérées (cf. TEST-002).

Jalon: 0.1
Feature: pipeline-sources
Refs: ADR-007, REQ-SW-022
```

```text
docs(contributing): documente la convention de commit

Fige le format des messages de commit : Conventional Commits enrichi des
trailers Jalon et Feature, afin d'identifier l'étape et la fonctionnalité
de chaque commit.

Jalon: hors-jalon
Feature: conventions
Refs: ADR-008
```

### Mauvais

```text
mise à jour divers
```
> Pas de type, pas de zone, pas de corps, aucun contexte.

```text
fix: corrections
```
> Sujet vague, ni jalon ni feature, impossible de savoir ce qui est corrigé.

## 8. Checklist avant commit

- [ ] Un seul objet / une seule intention
- [ ] `type` et `zone` corrects
- [ ] Sujet ≤ 72 caractères, à l'impératif
- [ ] Corps présent si le commit n'est pas trivial (pourquoi + quoi + impact)
- [ ] Trailers `Jalon:` et `Feature:` renseignés
- [ ] `Refs:` renseigné si une décision / exigence / test est concerné
