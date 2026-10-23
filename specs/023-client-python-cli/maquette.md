---
status: draft
updated: 2026-10-23
---

# 023 — Maquette de la CLI

Sessions de terminal imaginées, pour valider la forme avant tout code. Toutes les
données sont fictives : préfixes de documentation (RFC 5737 : `192.0.2.0/24`,
`198.51.100.0/24`, `203.0.113.0/24` ; RFC 3849 : `2001:db8::/32`), domaines
`example.org` et `example.net`, noms inventés. Les UUID sont abrégés (`7c1e…`). `$` est
l'invite ; les sorties sont en français (profil `lang = "fr"`).

Deux serveurs fictifs servent de décor :
- `https://kartenn.example.org` (profil `prod`) : build 0.2.0, modules `vlan`,
  `machines`, `policy`, `flow` ;
- `https://kartenn.example.net` (profil `labo`) : build 0.1.4, modules `vlan`,
  `machines` seulement.

## 1. Première connexion

```
$ kartenn config add-profile prod --url https://kartenn.example.org --auth oidc --issuer interne
Profil « prod » créé ($HOME/.config/kartenn/config.toml) et choisi par défaut.

$ kartenn login
Ouverture du navigateur pour vous connecter chez « interne »…
  Si rien ne s'ouvre : https://idp.example.org/authorize?client_id=kartenn-cli&…
Connecté : Alice Exemple (interne), jeton valable jusqu'à 14:42, renouvelé automatiquement.
Découverte du serveur… 0.2.0, modules : vlan, machines, policy, flow (catalogue en cache).

$ kartenn me
Identité   Alice Exemple  (personne, émetteur interne)
Groupes    reseau-admins, securite-lecteurs
Liaisons   network-admin @ entité exemple ; policy-reader @ global
```

Sur un hôte sans navigateur :

```
$ kartenn login --device
Vous autorisez un APPAREIL à agir en votre nom sur kartenn (profil prod).
Ne saisissez ce code que si vous avez lancé cette commande vous-même.

  Ouvrez  https://idp.example.org/device
  Code    WDJB-MJHT

En attente de validation… (expire dans 10 min)
Connecté : Alice Exemple (interne).
```

## 2. Aide de la racine (serveur `prod`)

```
$ kartenn --help
Usage: kartenn [OPTIONS] COMMANDE [ARGS]...

  Client de kartenn — profil « prod », https://kartenn.example.org (0.2.0).

Options:
  -p, --profile NOM        Profil de configuration.
  -o, --output FORMAT      table | json | ndjson | yaml | csv  [défaut : table sur un terminal]
  --at DATE                Temps applicatif (valid_at) : 2026-11-01, -7d, RFC 3339.
  --as-of DATE             Temps système : ce que kartenn affirmait alors.
  --keys / --labels        Clés internes ou libellés.
  --lang [fr|en]           Langue des messages.
  --no-input               Jamais de question, de navigateur ni d'éditeur.
  -v, --verbose            Requêtes, durées, identifiants de requête.
  --version                Version du client.
  -h, --help               Cette aide.

Noyau:
  entities, sites, domains, prefixes, ranges, addresses, anomalies, property-declarations,
  property-gaps, graph

Écritures:
  changeset   Composer, simuler, appliquer, instruire des changesets.
  import      Importer un fichier par morceaux, avec reprise.

Modules servis:
  policy      Politique de flux : règles, matrice, catalogue, vérification.   [module policy 0.2.0]
  flow        Registre de flux : demandes, instruction, dérogations.          [module flow 0.2.0]
  vlan        VLAN et groupes de VLAN.                                         [module vlan 0.2.0]
  machines    Machines, interfaces, grappes.                                   [module machines 0.2.0]

Droits et audit:
  roles, bindings, issuer-attachments, service-accounts, identities, audit, me

Outils:
  api         Appeler n'importe quelle opération de l'API.
  capabilities, health, log-filters, login, logout, config, completion, stubs
```

## 3. Aide d'un module découvert

```
$ kartenn policy --help
Usage: kartenn policy [OPTIONS] COMMANDE [ARGS]...

  Politique de flux (module policy 0.2.0, servi par kartenn.example.org).
  Commandes dérivées du contrat du serveur ; « kartenn api » pour le reste.

Vérifier:
  check            Verdict d'une communication source → destination pour un service.
  report           Rapport d'audit de conformité (tâche de fond).

Écrire la politique:
  rules            Règles d'autorisation et d'interdiction.
  matrix           Matrice environnements × types de flux.
  patterns         Motifs (autorisations conditionnelles).
  axes, values     Axes de classification et leurs valeurs.
  services         Catalogue de services nommés.
  environments     Environnements.
  flow-types       Types de flux.
  inference-rules  Règles d'inférence de classification.
  risk-signals, admissibility-conditions, exempt-roles

$ kartenn policy rules list --help
Usage: kartenn policy rules list [OPTIONS]

  Lister les règles de politique.     GET|QUERY /v1/policy-rules  (list_policy_rules)

Filtres:
  --effect [allow|deny]          Effet (un parmi, répétable).
  --flow-type CLÉ                Type de flux (complétion depuis le serveur).
  --prop CLÉ[:OP]=VALEUR         Propriété : =, :between=a..b, :exists=true|false.
Dates:
  --at DATE  --as-of DATE
Sortie:
  -o FORMAT  --fields A,B  --query JMESPATH  --all  --page-size N
```

## 4. Lister, à une date, en table et en JSON

```
$ kartenn prefixes list --domain interne --within 198.51.100.0/24
DOMAINE  VALEUR             NATURE      ENVIRONNEMENT   SITE        VLAN
interne  198.51.100.0/24    agrégat     Production      site-nord   —
interne  198.51.100.0/26    sous-réseau Production      site-nord   110 admin
interne  198.51.100.64/26   sous-réseau Recette         site-nord   120 recette
interne  198.51.100.128/25  sous-réseau Développement   site-sud    130 dev
4 préfixes — état au 2026-10-23 10:14:03+02:00

$ kartenn prefixes list --domain interne --within 198.51.100.0/24 --keys
DOMAINE  VALEUR             NATURE      ENVIRONNEMENT  SITE       VLAN
interne  198.51.100.0/24    aggregate   prod           site-nord  —
…

$ kartenn prefixes list --domain interne --within 198.51.100.0/24 --at 2026-06-01 --as-of 2026-07-01
DOMAINE  VALEUR             NATURE      ENVIRONNEMENT   SITE        VLAN
interne  198.51.100.0/24    agrégat     Production      site-nord   —
interne  198.51.100.0/25    sous-réseau Production      site-nord   110 admin
2 préfixes — valide au 2026-06-01 00:00+02:00, tel qu'affirmé le 2026-07-01 00:00+02:00

$ kartenn prefixes list --domain interne --prop policy.environment=prod -o json --fields value,properties.policy.environment
[
  {"value": "198.51.100.0/24", "properties": {"policy.environment": "prod"}},
  {"value": "198.51.100.0/26", "properties": {"policy.environment": "prod"}}
]

$ kartenn prefixes list --domain interne -o ndjson --all | wc -l
48211
```

## 5. Complétion

```
$ kartenn prefixes list --domain <TAB>
dmz  interne  labo  public
$ kartenn prefixes list --domain interne --prop <TAB>
core.owner=  core.site=  dns.ttl=  policy.environment=  policy.sensitivity=
$ kartenn prefixes list --domain interne --prop policy.environment=<TAB>
prod  -- Production
recette  -- Recette
dev  -- Développement
$ kartenn policy check --service <TAB>
dns  https  ldaps  ntp  ssh  syslog-tls
$ kartenn vlan <TAB>
close  create  edit  get  groups  history  list  prefixes  properties  update
```

## 6. Vérifier une communication (policy check)

```
$ kartenn policy check machine:bastion-1 prefix:interne/198.51.100.128/25 --service ssh
Verdict  ✓ autorisé — issue automatique
  Source       bastion-1 (192.0.2.10, interne)   Administration · Production
  Destination  198.51.100.128/25 (interne)       Développement
  Service      ssh (tcp/22)
  Règle        [allow] admin-vers-tout  « L'administration atteint tous les environnements »
  Conditions   toutes remplies
  État au 2026-10-23 10:15:40+02:00

$ kartenn policy check address:interne/198.51.100.140 address:interne/198.51.100.10 --service ssh --exit-status
Verdict  ✗ refusé
  Source       198.51.100.140 (interne)  Développement
  Destination  198.51.100.10 (interne)   Production
  Règle        [deny] dev-vers-prod  « Aucun flux du développement vers la production »
  Interdiction de niveau organisation : aucune dérogation possible.
$ echo $?
9
```

## 7. Demander un flux (flow request)

```
$ kartenn flow request \
    --source machine:app-recette-2 --destination prefix:interne/203.0.113.0/28 \
    --service https --entity exemple --ref ticketing:CHG-2048 \
    --justification "Appel de l'API de facturation de recette" --motive "nouvelle recette"
Évaluation  ◐ autorisé sous réserve — instruction requise
  Condition non remplie : flux-recette-tracé (« la référence doit être un changement approuvé »)
Demande enregistrée, à instruire : changeset 0c41…  (état : en attente)
  Suivre : kartenn changeset show 0c41…   Instructeurs : rôle flow-reviewer @ entité exemple
$ echo $?
0

$ kartenn flow list --state pending --entity exemple
ID      SOURCE          DESTINATION          SERVICE  ÉTAT        DEMANDEUR      RÉFÉRENCE
4f9a…   app-recette-2   203.0.113.0/28       https    à instruire Alice Exemple  ticketing:CHG-2048

$ kartenn flow review 4f9a… --approve --end 2027-04-30 --motive "validé en comité du 23/10"
Flux 4f9a… approuvé (fin 2027-04-30) — changeset 0c41… appliqué.
```

## 8. Simuler puis appliquer un changeset

```
$ kartenn changeset new --motive "Ouverture du lot 3" --valid-from 2026-11-01
Changeset local : ./.kartenn-changeset.yaml (0 opération)

$ kartenn prefixes create --domain interne --value 203.0.113.0/26 --nature subnet \
    --prop policy.environment=prod --into-changeset
Ajouté (opération 0) : CreatePrefix interne/203.0.113.0/26

$ kartenn vlan create --group site-nord --vid 140 --name lot3 --into-changeset
Ajouté (opération 1) : vlan.CreateVlan site-nord/140

$ kartenn changeset simulate
Simulation — rien n'est écrit (valide à partir du 2026-11-01 00:00+01:00)
  Opérations  2   Violations  0   Avertissements  1
  ! [core.prefix.unattached] /operations/0  203.0.113.0/26 n'est rattaché à aucun VLAN

Différentiel
  noyau            +1 préfixe
  vlan             +1 VLAN
  policy.combinations   3 combinaisons ouvertes
    SOURCE            DESTINATION        SERVICE  AVANT    APRÈS
    Administration    203.0.113.0/26     ssh      —        autorisé
    Supervision       203.0.113.0/26     syslog   —        autorisé
    Recette           203.0.113.0/26     https    —        refusé
  flow.flows       aucun flux touché

$ kartenn changeset apply
Changeset 9d27… appliqué le 2026-10-23 10:21:07+02:00 (2 opérations).
```

Écriture directe avec `--dry-run`, puis refus :

```
$ kartenn prefixes create --domain interne --value 203.0.113.32/27 --nature subnet --dry-run
Simulation — rien n'est écrit
erreur [changeset-refused] Changeset refusé (422)
  1 violation — requête urn:uuid:5b0e…
  ✗ [core.prefix.overlap] /value  203.0.113.32/27 chevauche 203.0.113.0/26 (interne), valide depuis le 2026-11-01
$ echo $?
8
```

## 9. Modifier par l'éditeur, conflit de version

```
$ kartenn vlan edit site-nord/140
# (éditeur : YAML de l'objet ; « id », « href », « vid » en commentaire, non modifiables)
Modifications : name: lot3 → lot3-prod ; properties.policy.environment: — → prod
Motif ? Renommage après mise en service
Changeset 1aa2… appliqué.

$ kartenn vlan update site-nord/140 --set name=lot3-bis --motive test --no-input
erreur [precondition-failed] L'objet a changé depuis sa lecture (412)
  Version lue 9d27…, version courante 1aa2… (requête urn:uuid:c3d1…)
  Relisez l'objet, ou utilisez « kartenn vlan edit » pour fusionner.
$ echo $?
5
```

## 10. Importer un fichier, avec avancement et erreur codée

```
$ head -3 adresses.csv
domain,value,usage,prop.dns.fqdn,external_id
interne,198.51.100.21,host,web-1.example.org,src-0001
interne,198.51.100.22,host,web-2.example.org,src-0002

$ kartenn import adresses.csv --resource addresses --id-column external_id --source cmdb-export
Import 61b0… (source cmdb-export) — 120 000 éléments, morceaux de 5 000
  Envoi   ██████████████████████████████  24/24 morceaux  120 000 él.  9 800 él./s
  Tri     ██████████████████░░░░░░░░░░░░  lots 14/24
  Lot 15  refusé
erreur [invalid-import] L'import n'a pas pu être trié (422)
  1 violation — import 61b0…  requête urn:uuid:7e44…
  ✗ [import.unknown-reference] adresses.csv:73 412 (élément 73 410, /operation/domain)
    Le domaine « interne-old » est inconnu à la date visée.
  L'import est revenu à l'état « envoi » : corrigez puis « kartenn import --resume 61b0… adresses.csv ».
$ echo $?
6

$ kartenn import --resume 61b0… adresses.csv
Empreinte du fichier modifiée : renvoi des morceaux 15 à 24 (les morceaux 1 à 14 sont inchangés).
  Envoi   ██████████████████████████████  10/10 morceaux
  Tri     ██████████████████████████████  24/24 lots
  Application ███████████████████████████  terminé en 2 min 41 s
Import 61b0… appliqué : 120 000 éléments, 24 changesets.
```

Hors terminal, une ligne par étape :

```
$ kartenn import adresses.ndjson --source cmdb-export 2>&1 | cat
import 61b0… created
import 61b0… uploaded chunks=24 elements=120000
import 61b0… sorting
import 61b0… applied changesets=24 elapsed=161s
```

## 11. Commande générique

```
$ kartenn api GET /v1/prefixes/7c1e…/next-free -f kind=prefix -f length=28
{"value": "203.0.113.64/28", "domain": {"id": "3f02…", "slug": "interne"}}

$ kartenn api QUERY /v1/audit-events --data '{"filters":[{"field":"actor","op":"eq","value":"sync-dns"}]}' --paginate -o ndjson | head -2
{"name": "changeset.applied", "actor": "sync-dns", "at": "2026-10-23T09:00:02+02:00", …}
{"name": "changeset.applied", "actor": "sync-dns", "at": "2026-10-23T08:00:01+02:00", …}
```

## 12. Serveur sans le module `policy`

```
$ kartenn -p labo --help | sed -n '/Modules servis/,/^$/p'
Modules servis:
  vlan        VLAN et groupes de VLAN.          [module vlan 0.1.4]
  machines    Machines, interfaces, grappes.    [module machines 0.1.4]

$ kartenn -p labo policy check machine:bastion-1 prefix:interne/198.51.100.0/24 --service ssh
erreur [client.module-unavailable] Le module « policy » n'est pas servi par ce serveur.
  Profil labo — https://kartenn.example.net, version 0.1.4
  Modules servis : vlan, machines
  La commande existe quand le serveur sert le module ; aucune mise à jour du client n'est nécessaire.
$ echo $?
2

$ kartenn -p labo addresses flows 2b10…
erreur [client.module-unavailable] « addresses flows » est servi par le module « flow », absent de ce serveur.
```

Après l'ajout du module `policy` au serveur `labo` (nouvelle build 0.2.0) :

```
$ kartenn -p labo policy --help | head -3
Découverte du serveur… 0.2.0 (avant 0.1.4), modules : vlan, machines, policy (catalogue mis à jour).
Usage: kartenn policy [OPTIONS] COMMANDE [ARGS]...
  Politique de flux (module policy 0.2.0, servi par kartenn.example.net).
```

## 13. Compte de service dans un script

```
$ export KARTENN_PROFILE=ci KARTENN_SECRET_FILE=/run/secrets/sync-dns
$ kartenn addresses list --domain interne --prop dns.fqdn:exists=true -o csv --fields value,properties.dns.fqdn > fqdn.csv
$ kartenn prefixes close interne/192.0.2.0/27 --no-input
erreur [client.motive-required] Une clôture exige --motive (le serveur répondrait motive-required).
$ echo $?
2
```

## 14. Bibliothèque, même scénario

```python
from kartenn import Client, Refused

with Client.from_profile("prod") as kn:
    v = kn.call("show_verdicts", source="machine:bastion-1",       # toute opération par son operationId
                destination="prefix:interne/198.51.100.128/25", service="ssh")
    print(v["verdict"], v["outcome"])                  # allowed automatic

    cs = kn.changeset(motive="Ouverture du lot 3", valid_from="2026-11-01")
    cs.create("prefixes", domain={"slug": "interne"}, value="203.0.113.0/26",
              nature="subnet", properties={"policy.environment": "prod"})
    sim = cs.simulate()
    for row in sim.section("policy.combinations"):
        print(row["source"], row["destination"], row["after"])
    try:
        cs.submit()
    except Refused as e:
        for viol in e.violations:
            print(viol.rule, viol.pointer, viol.detail)  # le code toujours disponible
```
