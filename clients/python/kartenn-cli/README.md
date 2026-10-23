# kartenn-cli

Ligne de commande `kartenn` de kartenn, référentiel réseau déclaratif. Ses commandes sont dérivées du contrat de l'API du serveur auquel elle
parle : les collections du noyau et celles des modules servis (politique de flux,
registre de flux, VLAN, machines…) apparaissent sans nouvelle version de la CLI.

## Installation

Python 3.11 ou plus récent.

```
pipx install kartenn-cli        # ou : uv tool install kartenn-cli
```

Avec Nix, depuis le flake du dépôt : `nix profile install .#kartenn-cli` (complétions
bash, zsh et fish, page de manuel `kartenn(1)` comprises). Sinon, la complétion
s'obtient par `kartenn completion bash|zsh|fish`.

## Premiers pas

```
kartenn config add-profile prod --url https://kartenn.example.org --issuer interne
kartenn login                   # navigateur ; --device sur un hôte sans navigateur
kartenn --help                  # commandes du serveur découvert
kartenn prefixes list --domain interne --at 2026-11-01
kartenn policy check prefix:interne/198.51.100.0/26 prefix:interne/198.51.100.128/25 --service ssh
kartenn changeset new --motive "Lot 3"; kartenn changeset simulate; kartenn changeset apply
kartenn api GET /v1/capabilities
```

Les jetons vont au trousseau du système, ou dans un fichier explicite
(`--token-file`) ; un script utilise un compte de service (`--auth basic`,
`KARTENN_SECRET_FILE`) ou `KARTENN_TOKEN`. Les codes de sortie sont stables
(2 usage, 4 authentification, 6 refus, 8 simulation refusée, 9 verdict refusé…).

## Licence

AGPL-3.0-only (fichier `LICENSE`). La bibliothèque sur laquelle elle repose,
[`kartenn`](https://pypi.org/project/kartenn/), est sous MPL-2.0.
