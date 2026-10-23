#!/usr/bin/env dash
# Wheels of the Python clients (spec 023, task 3; REQ-023-42): builds the wheel of each
# package with uv, then checks that it is pure (`py3-none-any`) and carries its licence:
# the SPDX expression of its metadata and the licence text in its dist-info.
#
#   scripts/py-wheels.sh [output directory]    (from the devenv shell: uv, python)
set -uo pipefail
cd "$(dirname "$0")/../clients/python" || exit 1

out=${1:-$(mktemp -d -t kartenn-wheels.XXXXXX)}
mkdir -p "$out" || exit 1

# check <distribution> <SPDX expression>
check() {
  python -I - "$out" "$1" "$2" <<'EOF'
import glob, sys, zipfile
from email.parser import HeaderParser

out, dist, spdx = sys.argv[1:]
wheels = glob.glob(f"{out}/{dist.replace('-', '_')}-*-py3-none-any.whl")
if len(wheels) != 1:
    sys.exit(f"KO {dist} : une roue py3-none-any attendue dans {out}, {len(wheels)} trouvée(s)")
with zipfile.ZipFile(wheels[0]) as wheel:
    names = wheel.namelist()
    metadata = next(n for n in names if n.endswith(".dist-info/METADATA"))
    headers = HeaderParser().parsestr(wheel.read(metadata).decode())
expression = headers.get_all("License-Expression") or []
if expression != [spdx]:
    sys.exit(f"KO {dist} : License-Expression {expression}, {spdx} attendue")
if not any(n.endswith(".dist-info/licenses/LICENSE") for n in names):
    sys.exit(f"KO {dist} : texte de licence absent de la roue")
print(f"{wheels[0].rsplit('/', 1)[1]} : {spdx}")
EOF
}

for pkg in kartenn kartenn-cli; do
  (cd "$pkg" && uv build --wheel -o "$out") >"$out/build-$pkg.log" 2>&1 || {
    echo "KO $pkg : uv build ($out/build-$pkg.log)"
    tail -5 "$out/build-$pkg.log"
    exit 1
  }
done
check kartenn MPL-2.0 && check kartenn-cli AGPL-3.0-only || exit 1
echo "OK — roues de kartenn et kartenn-cli ($out)"
