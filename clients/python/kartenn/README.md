# kartenn

Bibliothèque cliente Python de l'API de kartenn, référentiel réseau déclaratif. Elle
découvre ce que sert le serveur depuis son contrat (`/v1/openapi.json`) et ses
capacités : les collections du noyau et celles des modules (politique de flux, registre
de flux, VLAN, machines…) s'utilisent sans nouvelle version de la bibliothèque.

## Installation

Python 3.11 ou plus récent ; dépendances d'exécution : `httpx2`, `platformdirs`.

```
pip install kartenn             # ou : uv add kartenn
```

Avec Nix, depuis le flake du dépôt : le paquet `kartenn-py`.

## Exemple

```python
from kartenn import Client, Refused

with Client.from_profile("prod") as kn:
    v = kn.call("show_verdicts", source="prefix:<id>", destination="prefix:<id>", service="ssh")
    cs = kn.changeset(motive="Ouverture du lot 3", valid_from="2026-11-01")
    cs.create("prefixes", domain={"slug": "interne"}, value="203.0.113.0/26", nature="network")
    print(cs.simulate().violations)
    try:
        cs.submit()
    except Refused as e:
        for violation in e.violations:
            print(violation.rule, violation.pointer, violation.detail)
```

Un client asynchrone (`AsyncClient`) offre les mêmes opérations. L'authentification
couvre OIDC natif (navigateur et bouclage, flux d'appareil), comptes de service (Basic),
jeton porteur et compte de secours ; les secrets passent par un `TokenStore`.

## Licence

MPL-2.0 (fichier `LICENSE`).
