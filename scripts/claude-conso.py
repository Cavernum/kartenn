#!/usr/bin/env python3
"""Consommation de tokens des sessions Claude Code de ce dépôt (sous-agents inclus).

    scripts/claude-conso.py [nombre de sessions, défaut 5]

Par session : requêtes, tokens lus et écrits en cache, sortie, contexte moyen et p90,
réécritures de cache > 100 k après plus d'une heure d'inactivité, et les prompts les
plus coûteux. Sert à vérifier l'effet des règles « Économie de contexte » du CLAUDE.md.
"""
import json
import statistics
import sys
from datetime import datetime
from pathlib import Path

root = Path.home() / ".claude/projects" / str(Path(__file__).resolve().parents[1]).replace("/", "-")


def ts(d):
    return datetime.fromisoformat(d["timestamp"].replace("Z", "+00:00"))


def usage(path):
    seen, rows = set(), []
    prompt = None
    for raw in path.open():
        d = json.loads(raw)
        if d.get("type") == "user" and isinstance(d.get("message", {}).get("content"), str) and not d.get("isMeta"):
            prompt = d["message"]["content"][:70].replace("\n", " ")
        if d.get("type") != "assistant" or d["message"].get("id") in seen:
            continue
        seen.add(d["message"].get("id"))
        u = d["message"].get("usage") or {}
        rows.append((ts(d), u.get("cache_read_input_tokens") or 0, u.get("cache_creation_input_tokens") or 0,
                     u.get("output_tokens") or 0, (u.get("input_tokens") or 0), prompt, d["message"].get("model")))
    return rows


def report(main):
    rows = usage(main)
    if not rows:
        return
    subs = sorted((main.parent / main.stem / "subagents").glob("*.jsonl"))
    ctx = [r[1] + r[2] + r[4] for r in rows]
    cold = [(r[0], r[2]) for prev, r in zip(rows, rows[1:]) if r[2] > 100_000 and (r[0] - prev[0]).total_seconds() > 3600]
    per = {}
    for r in rows:
        per[r[5]] = per.get(r[5], 0) + r[1] + r[2]
    print(f"== {main.stem}  {rows[0][0]:%Y-%m-%d %H:%M} → {rows[-1][0]:%Y-%m-%d %H:%M}")
    print(f"   {len(rows)} requêtes · lu {sum(r[1] for r in rows)/1e6:.1f} M · écrit {sum(r[2] for r in rows)/1e6:.1f} M"
          f" · sortie {sum(r[3] for r in rows)/1e6:.2f} M · ctx moyen {statistics.mean(ctx)/1e3:.0f} k"
          f" · p90 {sorted(ctx)[int(len(ctx)*.9)]/1e3:.0f} k")
    print(f"   réécritures froides : {len(cold)} ({sum(c[1] for c in cold)/1e6:.1f} M)")
    for s in subs:
        sr = usage(s)
        models = {r[6] for r in sr} - {None}
        print(f"   sous-agent {s.stem[-8:]} {','.join(sorted(models))} : {len(sr)} req, lu {sum(r[1] for r in sr)/1e6:.1f} M")
    for p, v in sorted(per.items(), key=lambda x: -x[1])[:5]:
        print(f"   {v/1e6:6.1f} M  {p}")


n = int(sys.argv[1]) if len(sys.argv) > 1 else 5
for f in sorted(root.glob("*.jsonl"), key=lambda p: p.stat().st_mtime)[-n:]:
    report(f)
