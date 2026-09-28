#!/usr/bin/env python3
"""Check the STRATZ document this backend sends, against the live API.

Why this exists as a script rather than a test: the GraphQL document in
``services::match_facts::stratz`` is the one thing in that module a unit test
cannot prove. A fixture proves the normalizer reads the shape it is handed; only
the real endpoint proves STRATZ still *sends* that shape. A field renamed
upstream comes back as ``null``, and in a fixture test that is indistinguishable
from a field the provider legitimately does not have for that match.

Usage::

    ./scripts/verify_stratz.py <match_id> <account_id>

Both ids are yours: any match you played, and your Dota account id — the 32-bit
one rather than SteamID64, which ``GET /api/players/me`` reports.

The token comes from ``STRATZ_API_TOKEN`` in the environment, or from the repo's
``.env``. Nothing is written anywhere; this only reads.

Standard library only, so it runs wherever the repo is checked out.
"""

from __future__ import annotations

import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Kept in step with MATCH_FIELDS in
# backend/src/services/match_facts/stratz.rs. Changing one means changing both —
# the point of the script is that these are the same fields.
MATCH_FIELDS = """
    id
    didRadiantWin
    durationSeconds
    startDateTime
    parsedDateTime
    towerDeaths { time isRadiant }
    playbackData { roshanEvents { time } }
    players {
      steamAccountId
      isRadiant
      isVictory
      heroId
      lane
      position
      kills
      deaths
      assists
      goldPerMinute
      experiencePerMinute
      numLastHits
      numDenies
      networth
      level
      heroDamage
      towerDamage
      heroHealing
      stats {
        networthPerMinute
        lastHitsPerMinute
        deathEvents {
          time
          attacker
          goldLost
          goldFed
          timeDead
          isBurst
          hasHealAvailable
          isEngagedOnDeath
          isAttemptTpOut
        }
        itemPurchases { time itemId }
      }
    }
"""

# Every death field the normalizer reads. Printed with a verdict each, because
# "which of these came back null on a parsed replay" is the only question this
# script exists to answer.
DEATH_FIELDS = [
    "time",
    "attacker",
    "goldLost",
    "goldFed",
    "timeDead",
    "isBurst",
    "hasHealAvailable",
    "isEngagedOnDeath",
    "isAttemptTpOut",
]


def token() -> str:
    from_env = os.environ.get("STRATZ_API_TOKEN", "").strip()
    if from_env:
        return from_env

    env_file = ROOT / ".env"
    if env_file.is_file():
        for line in env_file.read_text().splitlines():
            match = re.match(r"^\s*STRATZ_API_TOKEN\s*=\s*(.*)$", line)
            if match:
                return match.group(1).strip().strip("'\"")

    sys.exit(
        f"STRATZ_API_TOKEN is not set (environment or {env_file}).\n"
        "Get one at https://stratz.com/api"
    )


def post(url: str, agent: str, bearer: str, query: str) -> dict:
    request = urllib.request.Request(
        url,
        data=json.dumps({"query": query}).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {bearer}",
            # STRATZ identifies API traffic by user agent and rejects some
            # clients without it.
            "User-Agent": agent,
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.loads(response.read())
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")[:500]
        sys.exit(f"HTTP {e.code} from STRATZ:\n{body}")
    except urllib.error.URLError as e:
        sys.exit(f"could not reach {url}: {e.reason}")


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(f"usage: {sys.argv[0]} <match_id> <account_id>")

    try:
        match_id = int(sys.argv[1])
        account_id = int(sys.argv[2])
    except ValueError:
        sys.exit("both ids must be integers")

    url = os.environ.get("STRATZ_API_URL", "https://api.stratz.com/graphql")
    agent = os.environ.get("STRATZ_USER_AGENT", "STRATZ_API")

    # An integer literal rather than a variable, exactly as the provider builds
    # it — a variable would need its scalar type spelled, and the point here is
    # to send what the backend sends.
    payload = post(url, agent, token(), f"{{ match(id: {match_id}) {{{MATCH_FIELDS}}} }}")

    # GraphQL reports failures per field, so errors can arrive beside usable
    # data. Both are worth seeing.
    for error in payload.get("errors") or []:
        print(f"! STRATZ reported: {error.get('message')}", file=sys.stderr)

    match = (payload.get("data") or {}).get("match")
    if not match:
        sys.exit(f"STRATZ returned no match {match_id}.")

    parsed = match.get("parsedDateTime") is not None
    print("--- match ---")
    for key in ("id", "didRadiantWin", "durationSeconds", "startDateTime"):
        print(f"  {key}: {match.get(key)!r}")
    print(f"  parsed replay: {parsed}")
    print(f"  towerDeaths: {len(match.get('towerDeaths') or [])}")
    roshan = ((match.get("playbackData") or {}).get("roshanEvents")) or []
    print(f"  roshanEvents: {len(roshan)}")

    players = match.get("players") or []
    print(f"  players: {len(players)}")

    me = next((p for p in players if p.get("steamAccountId") == account_id), None)
    if me is None:
        ids = [p.get("steamAccountId") for p in players]
        sys.exit(
            f"account {account_id} is not in this match. Accounts present: {ids}\n"
            "A null id is an anonymous profile, which the backend reads as "
            "'not in this match' by design."
        )

    print("\n--- you ---")
    for key in (
        "heroId",
        "isRadiant",
        "isVictory",
        "lane",
        "position",
        "kills",
        "deaths",
        "assists",
        "goldPerMinute",
        "experiencePerMinute",
        "numLastHits",
        "numDenies",
        "networth",
        "level",
        "heroDamage",
        "towerDamage",
        "heroHealing",
    ):
        print(f"  {key}: {me.get(key)!r}")

    stats = me.get("stats") or {}
    deaths = stats.get("deathEvents") or []
    print(f"  deathEvents: {len(deaths)}")
    print(f"  itemPurchases: {len(stats.get('itemPurchases') or [])}")
    print(f"  networthPerMinute: {len(stats.get('networthPerMinute') or [])} entries")
    print(f"  lastHitsPerMinute: {len(stats.get('lastHitsPerMinute') or [])} entries")

    print("\n--- death event fields ---")
    if not deaths:
        print("  no death events" + ("" if parsed else " (expected: replay not parsed)"))
    else:
        first = deaths[0]
        for field in DEATH_FIELDS:
            present = field in first and first[field] is not None
            mark = "ok  " if present else "NULL"
            print(f"  [{mark}] {field}: {first.get(field)!r}")

    # The catalogue call, which resolves hero and item names for the timeline.
    names = post(
        url,
        agent,
        token(),
        "{ constants { heroes { id displayName } items { id displayName } } }",
    )
    constants = (names.get("data") or {}).get("constants") or {}
    print("\n--- catalogue ---")
    print(f"  heroes: {len(constants.get('heroes') or [])}")
    print(f"  items: {len(constants.get('items') or [])}")

    print(
        "\nEvery key above is one the normalizer reads. A NULL on a parsed replay"
        "\nis the signal to check STRATZ's schema for a rename."
    )


if __name__ == "__main__":
    main()
