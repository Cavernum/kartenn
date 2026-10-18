#!/usr/bin/env python3
"""État d'une session Claude Code, lu à la fin de son transcript (JSONL).

Usage : etat-session.py <transcript>
Affiche « <contexte en tokens> <secondes depuis la dernière réponse> <epoch du dernier
prompt utilisateur> » ; 0 pour une valeur inconnue. Ne lit que la fin du fichier : un
transcript de session longue dépasse plusieurs dizaines de Mo.
"""
import json
import os
import sys
import time
from datetime import datetime

TAIL = 4 * 1024 * 1024


def epoch(ts: str) -> float:
    return datetime.fromisoformat(ts.replace("Z", "+00:00")).timestamp()


def main() -> None:
    ctx = idle = prompt = 0
    path = sys.argv[1] if len(sys.argv) > 1 else ""
    try:
        with open(path, "rb") as f:
            f.seek(max(0, os.path.getsize(path) - TAIL))
            lines = f.read().splitlines()
    except OSError:
        lines = []
    for raw in reversed(lines):
        try:
            d = json.loads(raw)
        except ValueError:
            continue
        kind = d.get("type")
        if not ctx and kind == "assistant" and not d.get("isSidechain"):
            u = d.get("message", {}).get("usage") or {}
            total = sum(u.get(k) or 0 for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"))
            if total:
                ctx = total
                idle = int(time.time() - epoch(d["timestamp"]))
        if not prompt and kind == "user" and not d.get("isMeta") and not d.get("isSidechain"):
            content = d.get("message", {}).get("content")
            if isinstance(content, str) and d.get("timestamp"):
                prompt = int(epoch(d["timestamp"]))
        if kind == "system" and d.get("subtype") == "compact_boundary" and not ctx:
            break  # compacté depuis la dernière réponse : contexte inconnu, petit
        if ctx and prompt:
            break
    print(ctx, idle, prompt)


main()
