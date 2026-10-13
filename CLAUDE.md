# kartenn — consignes de travail

- **Specs avant le code.** Toute nouvelle brique ou décision technique est d'abord
  discutée et écrite (`specs/NNN-slug/` pour une fonctionnalité, `docs/adr/` pour une
  décision d'architecture, `docs/spec/` pour la spec système). On n'implémente qu'à
  partir d'une spec validée par l'utilisateur.
- **Décisions de référence** : `specs/001-cadrage/decisions.md` (D1…D37). Toute
  proposition qui contredit une décision le signale et demande un arbitrage.
- **Sources primaires** : les notes de `docs/reference/` s'appuient sur les RFC
  (rfc-editor.org), les drafts IETF (datatracker) et les documentations officielles.
  Chaque note cite URL et section, et distingue les exigences normatives
  (MUST/SHOULD, RFC 2119/8174) des choix propres à kartenn.
- **Confidentialité** : aucun nom d'organisation réelle, ni d'exemple tiré du SI de
  l'utilisateur, dans les specs ni dans la documentation.
- **Langue** : documentation en français pour l'instant (question ouverte Q-ouv-6).
- **Commits** : Conventional Commits, atomiques.
- **Environnement** : devenv (`devenv shell`, `devenv test`). PostgreSQL 18 via `devenv up`.
