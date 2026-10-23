#!/usr/bin/env bash
# Release of kartenn (ADR 0033), independent of any forge: the forge workflows only call it.
#
#   scripts/release.sh notes [X.Y.Z]        the message of the tag to come (default
#                                           version: bumped from the Conventional Commits)
#   scripts/release.sh prepare X.Y.Z [--skip-ci-check]
#                                           maintainer, locally: version, CHANGELOG.md,
#                                           commit `chore(release): vX.Y.Z`, signed tag,
#                                           then the push after a confirmation
#   scripts/release.sh verify vX.Y.Z        anyone (and the release workflow): annotated
#                                           tag, signature by a key of .github/release-keys,
#                                           commit on main, versions and CHANGELOG.md
#   scripts/release.sh tag-notes vX.Y.Z     body of the tag message (markdown, no signature)
#   scripts/release.sh artefacts vX.Y.Z <dir>
#                                           image archive, source archive, SHA256SUMS
#   scripts/release.sh push-image <archive> <registry/name:tag>
#                                           copy of the image archive, prints the digest
#
# Tags are created and signed by the maintainer only: no signing key ever reaches a CI.
# Environment:
#   KARTENN_REPO_URL       https URL of the repository for the links of the notes
#                          (default: the `upstream` remote, else `origin`)
#   K_RELEASE_REMOTES      remotes kept in sync and pushed (default: upstream and origin,
#                          those that exist)
#   K_RELEASE_SIGNING_KEY  key of `git tag -u` (default: user.signingkey)
#   K_RELEASE_MAIN_REF     reference of main for `verify` (default: origin/main)
set -euo pipefail
cd "$(dirname "$0")/.."

say() { printf '%s\n' "$*" >&2; }
die() {
  say "release : $*"
  exit 1
}
need() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || die "outil introuvable : $tool (lancer depuis \`devenv shell\`)"
  done
}

semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
check_version() { [[ $1 =~ $semver ]] || die "version invalide : « $1 » (attendu : X.Y.Z)"; }
check_tag() { [[ $1 == v* ]] && [[ ${1#v} =~ $semver ]] || die "étiquette invalide : « $1 » (attendu : vX.Y.Z)"; }

# The newest release tag, by version (empty without one).
last_tag() { git tag -l 'v*' | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -1 || true; }
# version_gt A B: A > B.
version_gt() { [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -1)" = "$1" ]; }

# https URL of a git remote (scp-like, ssh:// or https://), without `.git`.
remote_url() {
  local url
  url=$(git remote get-url "$1" 2>/dev/null) || return 1
  url=${url%.git}
  case $url in
    https://*) ;;
    ssh://*) url=${url#ssh://}; url=${url#*@}; url=https://${url/:[0-9]*\//\/} ;;
    *@*:*) url=${url#*@}; url=https://${url/://} ;;
    *) return 1 ;;
  esac
  printf '%s\n' "$url"
}
repo_url() {
  if [ -n "${KARTENN_REPO_URL+x}" ]; then
    printf '%s\n' "$KARTENN_REPO_URL"
  else
    remote_url upstream || remote_url origin || true
  fi
}

# Lines of CHANGELOG.md (or of stdin with `-`) under the `## [` headings whose text
# contains <needle> at the start (`## [Non publié]`, `## [X.Y.Z]`), blank lines trimmed
# at both ends. With a third argument, only the headings that also contain it.
section_body() {
  local needle=$1 also=${2:-} file=${3:-CHANGELOG.md}
  awk -v needle="$needle" -v also="$also" '
    /^## \[/ { inside = index($0, needle) == 1 && (also == "" || index($0, also) > 0); next }
    inside { print }
  ' "$file" | sed -e '/./,$!d' | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}'
}

# The hand-written highlights of version <X.Y.Z>: the `## [Non publié]` section, and a
# `## [X.Y.Z] — à publier` section prepared ahead, if any.
highlights() {
  {
    section_body "## [$1]" "à publier"
    printf '\n'
    section_body "## [Non publié]"
  } | sed -e '/./,$!d' | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}'
}

# The CHANGELOG.md section of version <X.Y.Z>, rendered by git-cliff from the commits since
# the last tag.
render_section() {
  local version=$1
  local log out
  log=$(mktemp)
  if ! out=$(KARTENN_REPO_URL=$(repo_url) \
    KARTENN_RELEASE_HIGHLIGHTS=$(highlights "$version") \
    KARTENN_RELEASE_DATE=${KARTENN_RELEASE_DATE:-$(date -u +%F)} \
    git cliff --config cliff.toml --unreleased --tag "v$version" 2>"$log"); then
    cat "$log" >&2
    rm -f "$log"
    die "git cliff : échec"
  fi
  rm -f "$log"
  printf '%s\n' "$out" | sed -e '/./,$!d' -e 's/[[:space:]]*$//'
}

# The tag message, from the section body on stdin (without its heading line): the subject
# `vX.Y.Z`, a blank line, then the markdown body, which the GitHub release takes as its
# notes.
tag_message() {
  printf 'v%s\n\n' "$1"
  sed -e '/./,$!d'
}

# The workspace version, read from a Cargo.toml on stdin.
cargo_version() {
  awk '
    /^\[/ { section = $0 }
    section == "[workspace.package]" && /^version *=/ { gsub(/.*= *"|".*/, ""); print; exit }
  '
}

cmd_notes() {
  need git git-cliff
  local version=${1:-}
  if [ -z "$version" ]; then
    version=$(git cliff --config cliff.toml --bumped-version 2>/dev/null)
    version=${version#v}
    say "version calculée depuis les Conventional Commits : $version"
  fi
  check_version "$version"
  render_section "$version" | sed 1d | tag_message "$version"
}

cmd_tag_notes() {
  check_tag "$1"
  [ "$(git cat-file -t "refs/tags/$1" 2>/dev/null)" = tag ] || die "$1 : pas d'étiquette annotée"
  git for-each-ref --format='%(contents:body)' "refs/tags/$1" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}'
}

# Sets <version> in the workspace, the internal dependencies of the crates of kartenn, the
# web packages and Cargo.lock. The Nix package reads the first two (nix/package).
set_version() {
  local new=$1 old
  old=$(cargo_version <Cargo.toml)
  [ -n "$old" ] || die "Cargo.toml : version du workspace introuvable"
  [ "$old" != "$new" ] || return 0
  say "version : $old → $new"
  awk -v new="$new" '
    /^\[/ { section = $0 }
    section == "[workspace.package]" && /^version *=/ && !done { $0 = "version = \"" new "\""; done = 1 }
    { print }
  ' Cargo.toml >Cargo.toml.new && mv Cargo.toml.new Cargo.toml
  local manifest
  for manifest in crates/kartenn-*/Cargo.toml; do
    sed -E -i "/^kartenn-workspace-hack/! s/^(kartenn-[a-z0-9-]+ = \\{ version = )\"${old//./\\.}\"/\\1\"$new\"/" "$manifest"
  done
  if grep -nE "^kartenn-[a-z0-9-]+ = .*version = \"${old//./\\.}\"" crates/kartenn-*/Cargo.toml | grep -v '^[^:]*:[0-9]*:kartenn-workspace-hack'; then
    die "dépendances internes restées en $old (ci-dessus)"
  fi
  local package
  for package in web/sdk web/shell web/e2e; do
    sed -E -i "0,/^  \"version\": \"[^\"]*\",/s//  \"version\": \"$new\",/" "$package/package.json"
    [ "$(jq -r .version "$package/package.json")" = "$new" ] || die "$package/package.json : version non portée"
  done
  cargo update --workspace --offline --quiet
}

# Replaces `## [Non publié]` and a `## [X.Y.Z] — à publier` section by an empty
# `## [Non publié]` followed by the rendered section (file <section>).
write_changelog() {
  local version=$1 section=$2
  grep -q '^## \[Non publié\]' CHANGELOG.md || die "CHANGELOG.md : section « ## [Non publié] » absente"
  awk -v version="$version" -v section="$section" '
    /^## \[/ {
      skip = 0
      if (index($0, "## [Non publié]") == 1) {
        print; print ""
        while ((getline line < section) > 0) print line
        print ""
        skip = 1; next
      }
      if (index($0, "## [" version "]") == 1 && index($0, "à publier") > 0) { skip = 1; next }
    }
    !skip { print }
  ' CHANGELOG.md | cat -s >CHANGELOG.md.new && mv CHANGELOG.md.new CHANGELOG.md
}

# The fingerprints of the primary keys of .github/release-keys/*.asc.
release_fingerprints() {
  local key
  for key in .github/release-keys/*.asc; do
    [ -f "$key" ] || continue
    gpg --batch --with-colons --show-keys "$key" 2>/dev/null |
      awk -F: '$1 == "pub" { want = 1; next } $1 == "fpr" && want { print $10; want = 0 }'
  done
}

# Verifies the signature of <tag> against the keys of .github/release-keys only, in a
# throwaway keyring; prints the fingerprint of the signing primary key.
verify_signature() (
  tag=$1
  allowed=$(release_fingerprints)
  [ -n "$allowed" ] || die "aucune clé de publication dans .github/release-keys"
  GNUPGHOME=$(mktemp -d)
  export GNUPGHOME
  trap 'gpgconf --kill all 2>/dev/null; rm -rf "$GNUPGHOME"' EXIT
  chmod 700 "$GNUPGHOME"
  gpg --batch --quiet --import .github/release-keys/*.asc 2>/dev/null
  if ! status=$(git -c gpg.format=openpgp -c gpg.program=gpg verify-tag --raw "$tag" 2>&1); then
    say "$status"
    die "$tag : signature invalide ou clé absente de .github/release-keys"
  fi
  grep -q '^\[GNUPG:\] GOODSIG ' <<<"$status" || die "$tag : pas de signature valide (GOODSIG)"
  fpr=$(awk '$2 == "VALIDSIG" { print $12 }' <<<"$status")
  { [ -n "$fpr" ] && grep -qx "$fpr" <<<"$allowed"; } || die "$tag : signée par « $fpr », hors de .github/release-keys"
  printf '%s\n' "$fpr"
)

cmd_verify() {
  local tag=$1 version commit main fpr file package section body
  need git gpg jq
  check_tag "$tag"
  version=${tag#v}
  [ "$(git cat-file -t "refs/tags/$tag" 2>/dev/null)" = tag ] ||
    die "$tag : étiquette absente ou légère (une étiquette annotée et signée est attendue)"
  [ "$(git for-each-ref --format='%(contents:subject)' "refs/tags/$tag")" = "$tag" ] ||
    die "$tag : le sujet du message n'est pas « $tag » (étiquette faite hors de k-release ?)"
  fpr=$(verify_signature "$tag")
  commit=$(git rev-parse "$tag^{commit}")
  main=${K_RELEASE_MAIN_REF:-origin/main}
  git rev-parse -q --verify "$main^{commit}" >/dev/null || die "$main introuvable (git fetch)"
  git merge-base --is-ancestor "$commit" "$main" || die "$tag : le commit $commit n'est pas sur $main"
  # Versions at the tagged commit, whatever the working tree.
  [ "$(git show "$commit:Cargo.toml" | cargo_version)" = "$version" ] ||
    die "$tag : Cargo.toml du commit n'est pas en $version"
  for package in web/sdk web/shell web/e2e; do
    [ "$(git show "$commit:$package/package.json" | jq -r .version)" = "$version" ] ||
      die "$tag : $package/package.json du commit n'est pas en $version"
  done
  file=$(mktemp)
  git show "$commit:CHANGELOG.md" >"$file"
  grep -q "^## \[$version\]" "$file" || { rm -f "$file"; die "$tag : CHANGELOG.md sans section ## [$version]"; }
  grep "^## \[$version\]" "$file" | grep -q 'à publier' && { rm -f "$file"; die "$tag : CHANGELOG.md : section $version encore « à publier »"; }
  section=$(section_body "## [$version]" "" "$file" | sed -e 's/[[:space:]]*$//')
  rm -f "$file"
  body=$(cmd_tag_notes "$tag" | sed -e 's/[[:space:]]*$//')
  [ "$section" = "$body" ] || die "$tag : le message de l'étiquette diffère de la section $version de CHANGELOG.md"
  say "OK — $tag : étiquette annotée, signée par $fpr, commit $commit sur $main, versions et CHANGELOG.md en $version"
}

cmd_artefacts() {
  local tag=$1 out=$2 version image system
  need git nix sha256sum
  check_tag "$tag"
  version=${tag#v}
  [ "$(git rev-parse HEAD)" = "$(git rev-parse "$tag^{commit}")" ] || die "HEAD n'est pas le commit de $tag"
  [ -z "$(git status --porcelain --untracked-files=no)" ] || die "arbre de travail modifié"
  mkdir -p "$out"
  system=$(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')
  image=$(nix build --no-link --print-out-paths .#oci)
  cp "$image" "$out/kartenn-$version-oci-$system.tar.gz"
  git archive --format=tar.gz --prefix="kartenn-$version/" -o "$out/kartenn-$version-source.tar.gz" "$tag"
  (cd "$out" && rm -f SHA256SUMS && sha256sum -- kartenn-* >SHA256SUMS)
  cat "$out/SHA256SUMS"
}

cmd_push_image() {
  local archive=$1 dest=$2 digest
  need skopeo
  [ -f "$archive" ] || die "archive introuvable : $archive"
  digest=$(mktemp)
  skopeo copy --quiet --digestfile "$digest" "docker-archive:$archive" "docker://$dest" >&2
  cat "$digest"
  rm -f "$digest"
}

# Undo instructions, printed when `prepare` stops after changing something.
stage=none
recovery() {
  local code=$?
  case $stage in
    edited)
      say ""
      say "Arrêt avant le commit, rien n'est commité. Pour revenir à l'état initial :"
      say "  git restore --staged --worktree -- Cargo.toml Cargo.lock crates web CHANGELOG.md"
      ;;
    committed)
      say ""
      say "Le commit de version reste local, rien n'est poussé. Pour l'annuler :"
      say "  git reset --keep HEAD~1"
      ;;
    tagged | kept)
      say ""
      say "Le commit de version et l'étiquette restent locaux, rien n'est poussé. Pour les annuler :"
      say "  git tag -d v$VERSION"
      say "  git reset --keep HEAD~1"
      ;;
  esac
  return "$code"
}

cmd_prepare() {
  local skip_ci=0 arg
  VERSION=
  for arg in "$@"; do
    case $arg in
      --skip-ci-check) skip_ci=1 ;;
      -*) die "option inconnue : $arg" ;;
      *) [ -z "$VERSION" ] || die "une seule version"; VERSION=$arg ;;
    esac
  done
  [ -n "$VERSION" ] || die "usage : k-release <X.Y.Z> [--skip-ci-check]"
  VERSION=${VERSION#v}
  check_version "$VERSION"
  local tag=v$VERSION
  need git git-cliff gpg jq cargo
  [ "$skip_ci" -eq 1 ] || need gh

  # 1. The state of the repository.
  [ "$(git symbolic-ref --quiet --short HEAD || true)" = main ] || die "à faire depuis la branche main"
  [ -z "$(git status --porcelain)" ] || die "arbre de travail non propre (git status)"
  local remotes remote head
  remotes=${K_RELEASE_REMOTES:-$(for r in upstream origin; do git remote get-url "$r" >/dev/null 2>&1 && printf '%s ' "$r"; done)}
  [ -n "$remotes" ] || die "aucun dépôt distant (K_RELEASE_REMOTES)"
  head=$(git rev-parse HEAD)
  for remote in $remotes; do
    say "récupération de $remote…"
    git fetch --quiet --tags "$remote" || die "git fetch $remote : échec (une étiquette distante diffère-t-elle d'une locale ?)"
    [ "$(git rev-parse "$remote/main")" = "$head" ] ||
      die "main n'est pas au niveau de $remote/main (git pull, ou pousser main et attendre la CI)"
    [ -z "$(git ls-remote --tags "$remote" "refs/tags/$tag")" ] || die "$tag existe déjà sur $remote"
  done
  git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "$tag existe déjà localement"
  local last
  last=$(last_tag)
  [ -z "$last" ] || version_gt "$VERSION" "${last#v}" || die "$VERSION n'est pas supérieure à la dernière version, $last"

  # 2. The continuous integration of this very commit.
  if [ "$skip_ci" -eq 1 ]; then
    say ""
    say "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
    say "!!  --skip-ci-check : l'état de la CI du commit $head N'EST PAS vérifié."
    say "!!  La publication ne doit pas suivre sans une CI verte sur ce commit."
    say "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
    say ""
  else
    local slug runs
    slug=$(repo_url | sed -E 's#^https://[^/]+/##')
    [ -n "$slug" ] || die "dépôt GitHub introuvable pour la vérification de la CI (KARTENN_REPO_URL)"
    runs=$(gh run list --repo "$slug" --workflow ci.yml --commit "$head" --event push \
      --json status,conclusion,url --limit 20) || die "gh run list : échec (gh auth status ?)"
    [ "$(jq length <<<"$runs")" -gt 0 ] || die "aucune exécution de la CI sur $slug pour $head"
    [ "$(jq -r '.[0].status' <<<"$runs")" = completed ] || die "CI en cours sur $head : $(jq -r '.[0].url' <<<"$runs")"
    [ "$(jq -r '.[0].conclusion' <<<"$runs")" = success ] ||
      die "CI non verte sur $head ($(jq -r '.[0].conclusion' <<<"$runs")) : $(jq -r '.[0].url' <<<"$runs")"
    say "CI verte sur $head : $(jq -r '.[0].url' <<<"$runs")"
  fi

  # 3. Version, changelog, commit. From here, `recovery` says how to undo.
  trap recovery EXIT
  stage=edited
  local section message
  section=$(mktemp)
  message=$(mktemp)
  [ -n "$(highlights "$VERSION")" ] || say "note : aucun fait marquant écrit sous « ## [Non publié] », la liste des commits seule"
  render_section "$VERSION" >"$section"
  grep -q "^## \[$VERSION\] — " "$section" || die "git-cliff : section inattendue ($section)"
  set_version "$VERSION"
  write_changelog "$VERSION" "$section"
  git add -- Cargo.toml Cargo.lock crates/kartenn-*/Cargo.toml web/sdk/package.json web/shell/package.json web/e2e/package.json CHANGELOG.md
  git commit --quiet -m "chore(release): $tag"
  stage=committed

  # 4. The signed tag: `--cleanup=verbatim` keeps the `#` of the markdown headings, which
  # the default cleanup would strip as comments.
  section_body "## [$VERSION]" | tag_message "$VERSION" >"$message"
  local sign=(--sign)
  [ -z "${K_RELEASE_SIGNING_KEY:-}" ] || sign=(--local-user "$K_RELEASE_SIGNING_KEY")
  git tag "${sign[@]}" --cleanup=verbatim --file "$message" "$tag"
  stage=tagged
  git tag --verify "$tag" >/dev/null 2>&1 || die "git tag --verify $tag : échec"
  local fpr
  fpr=$(verify_signature "$tag")
  say "étiquette $tag signée par $fpr (clé de .github/release-keys)"
  rm -f "$section" "$message"

  # 5. The push, after an explicit confirmation.
  say ""
  git --no-pager log --oneline -1 >&2
  say ""
  say "Message de l'étiquette : git tag -n200 -l $tag ; notes : scripts/release.sh tag-notes $tag"
  say "Poussée prévue (main et l'étiquette ensemble, atomique, par dépôt) :"
  for remote in $remotes; do say "  git push --atomic $remote refs/heads/main refs/tags/$tag"; done
  local answer=
  if [ -t 0 ]; then
    read -r -p "Taper « $tag » pour pousser maintenant, autre chose pour s'arrêter : " answer
  fi
  if [ "$answer" != "$tag" ]; then
    stage=kept
    say "Rien n'est poussé : relire, puis lancer les commandes ci-dessus."
    exit 0
  fi
  for remote in $remotes; do
    git push --atomic "$remote" refs/heads/main "refs/tags/$tag" ||
      die "poussée vers $remote en échec ; les dépôts déjà poussés gardent $tag : relancer la commande pour $remote"
  done
  stage=pushed
  say "OK — $tag poussée ; le workflow de publication part de l'étiquette."
}

case ${1:-} in
  notes) shift; cmd_notes "$@" ;;
  prepare) shift; cmd_prepare "$@" ;;
  verify) [ $# -eq 2 ] || die "usage : release.sh verify vX.Y.Z"; cmd_verify "$2" ;;
  tag-notes) [ $# -eq 2 ] || die "usage : release.sh tag-notes vX.Y.Z"; cmd_tag_notes "$2" ;;
  artefacts) [ $# -eq 3 ] || die "usage : release.sh artefacts vX.Y.Z <répertoire>"; cmd_artefacts "$2" "$3" ;;
  push-image) [ $# -eq 3 ] || die "usage : release.sh push-image <archive> <registre/nom:étiquette>"; cmd_push_image "$2" "$3" ;;
  *) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//' >&2; exit 2 ;;
esac
