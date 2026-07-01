#!/usr/bin/env python3
"""Build the city-scoped Textropolis dictionary.

The game consumes a JSON object shaped as:

    { "word": [{ "definition": "...", "part_of_speech": "noun" }] }

This script keeps that contract while using ESDB/SCOWL as the spelling and
inflection gate and Open English WordNet as the primary definition source.
Only words playable from the configured city names are written to the shipped
dictionary.
"""

from __future__ import annotations

import argparse
import collections
import gzip
import json
import re
import zipfile
from dataclasses import dataclass
from pathlib import Path


CITIES = [
    "Phoenix",
    "Johannesburg",
    "Bangalore",
    "Buenos Aires",
    "Hyderabad",
    "Kuala Lumpur",
    "Mexico City",
    "Dusseldorf",
    "Khartoum",
    "Barcelona",
    "Philadelphia",
    "Rio de Janeiro",
    "Dar es Salaam",
    "Belo Horizonte",
    "Alexandria",
    "New York",
    "St. Petersburg",
    "Washington",
    "Istanbul",
    "Ho Chi Minh City",
    "Monterrey",
    "Los Angeles",
    "Melbourne",
    "Singapore",
    "Santiago",
    "San Francisco",
    "Houston",
]

SOURCE_NOTES = {
    "open_english_wordnet": {
        "name": "Open English WordNet 2025 core JSON",
        "url": "https://en-word.net/static/english-wordnet-2025-json.zip",
        "role": "definitions and part-of-speech labels",
        "license": "CC-BY 4.0",
    },
    "esdb": {
        "name": "English Speller Database / SCOWL rel-2026.02.25 en_US-large",
        "url": "https://github.com/en-wl/wordlist-diff/tree/rel-2026.02.25",
        "role": "vetted US spellings, inflections, and category filtering",
        "license": "MIT-like permission notice; see ESDB Copyright",
    },
    "manual": {
        "name": "Textropolis local curation",
        "url": "scripts/build_textropolis_dictionary.py",
        "role": "legacy carryover filters and small project-specific additions",
        "license": "project-local curation",
    },
}

WORD_RE = re.compile(r"^[a-z]+$")
POS_TOKEN_RE = re.compile(r"<([^>]+)>")

PART_OF_SPEECH = {
    "n": "noun",
    "v": "verb",
    "a": "adjective",
    "s": "adjective",
    "r": "adverb",
}

SCOWL_EXCLUDE_PATTERNS = [
    " roman-numerals",
    "<x>",
    "<abbr",
    "</abbr",
    "/abbr",
    " upper",
    "[upper]",
    "/upper",
    "[name]",
    "/name",
    "/person",
    "/place",
    "/surname",
    "/demonym",
    "[town]",
]

IRREGULAR_VERB_FORMS = {
    "arose",
    "ate",
    "awoke",
    "awoken",
    "began",
    "begun",
    "been",
    "bore",
    "born",
    "bought",
    "bred",
    "brought",
    "came",
    "caught",
    "did",
    "done",
    "drank",
    "drawn",
    "drew",
    "driven",
    "drove",
    "eaten",
    "fell",
    "felt",
    "fled",
    "flew",
    "flown",
    "forgot",
    "forgotten",
    "gave",
    "given",
    "gone",
    "got",
    "gotten",
    "grew",
    "grown",
    "had",
    "held",
    "kept",
    "knew",
    "known",
    "laid",
    "led",
    "left",
    "lost",
    "made",
    "met",
    "paid",
    "ran",
    "read",
    "rose",
    "run",
    "said",
    "sang",
    "sat",
    "saw",
    "seen",
    "sent",
    "set",
    "sold",
    "sung",
    "swam",
    "swum",
    "taken",
    "told",
    "took",
    "was",
    "went",
    "were",
    "won",
    "woke",
    "woken",
    "wore",
    "worn",
    "wrote",
    "written",
}

IRREGULAR_VERB_PAST_PARTICIPLES = {
    "awoken",
    "begun",
    "been",
    "born",
    "brought",
    "done",
    "drawn",
    "driven",
    "eaten",
    "flown",
    "forgotten",
    "given",
    "gone",
    "gotten",
    "grown",
    "known",
    "seen",
    "sung",
    "swum",
    "taken",
    "woken",
    "worn",
    "written",
}

IRREGULAR_NOUN_PLURALS = {
    "children",
    "dice",
    "feet",
    "geese",
    "lice",
    "men",
    "mice",
    "oxen",
    "people",
    "teeth",
    "women",
}

IRREGULAR_COMPARATIVE_FORMS = {
    "best",
    "better",
    "farther",
    "farthest",
    "further",
    "furthest",
    "least",
    "less",
    "more",
    "most",
    "worse",
    "worst",
}

MANUAL_DEFINITIONS = {
    "stan": [
        {
            "definition": "an extremely enthusiastic fan",
            "part_of_speech": "noun",
        },
        {
            "definition": "to be an extremely enthusiastic fan of someone or something",
            "part_of_speech": "verb",
        },
    ],
}

MANUAL_INCLUDE_CURRENT_WORDS = {
    "african",
    "afro",
    "allah",
    "arse",
    "asian",
    "beaner",
    "bleu",
    "dane",
    "eros",
    "lense",
    "oreo",
    "phon",
    "reuben",
    "satan",
    "thou",
    "tore",
}

MANUAL_INCLUDE_WORDS = set(MANUAL_DEFINITIONS) | MANUAL_INCLUDE_CURRENT_WORDS


@dataclass(frozen=True)
class Inflection:
    base: str
    part_of_speech: str
    relation: str


def sanitize(value: str) -> str:
    return "".join(ch.upper() for ch in value if ch.isascii() and ch.isalpha())


def is_subanagram(source: str, word: str) -> bool:
    available = collections.Counter(source)
    for ch in word.upper():
        if available[ch] <= 0:
            return False
        available[ch] -= 1
    return True


CITY_LETTERS = {city: sanitize(city) for city in CITIES}


def city_matches(word: str) -> list[str]:
    if len(word) < 4:
        return []
    return [city for city, letters in CITY_LETTERS.items() if is_subanagram(letters, word)]


def is_playable(word: str) -> bool:
    return bool(WORD_RE.fullmatch(word)) and bool(city_matches(word))


def add_definition(
    definitions: dict[str, list[dict[str, str]]],
    word: str,
    part_of_speech: str,
    definition: str,
) -> bool:
    definition = " ".join(definition.split())
    if not WORD_RE.fullmatch(word) or not definition:
        return False
    entry = {"definition": definition, "part_of_speech": part_of_speech}
    if entry in definitions[word]:
        return False
    definitions[word].append(entry)
    return True


def select_definitions(entries: list[dict[str, str]], limit: int = 6) -> list[dict[str, str]]:
    selected: list[dict[str, str]] = []
    seen_entries = set()
    seen_pos = set()

    def append(entry: dict[str, str]) -> None:
        key = (entry["part_of_speech"], entry["definition"])
        if key in seen_entries or len(selected) >= limit:
            return
        selected.append(entry)
        seen_entries.add(key)
        seen_pos.add(entry["part_of_speech"])

    for entry in entries:
        if entry["part_of_speech"] not in seen_pos:
            append(entry)
    for entry in entries:
        append(entry)
    return selected


def lowercase_words(path: Path) -> set[str]:
    return {line.strip() for line in path.read_text().splitlines() if WORD_RE.fullmatch(line.strip())}


def scowl_is_excluded(line: str) -> bool:
    haystack = f" {line.lower()}"
    return any(pattern in haystack for pattern in SCOWL_EXCLUDE_PATTERNS)


def scowl_has_legacy_evidence(line: str) -> bool:
    info = line.split(": ", 1)[0].lower()
    if "[stale]" in info:
        return False
    sizes = [int(size) for size in re.findall(r"(?<![a-z])(\d{2})(?![a-z])", info)]
    return bool(sizes) and min(sizes) <= 80


def scowl_poses(raw: str) -> list[str]:
    poses = []
    if re.search(r"(^|[_/])n($|[_/?])", raw):
        poses.append("noun")
    if re.search(r"(^|[_/])v($|[_/?])", raw) or raw.startswith("m") or "/m" in raw:
        poses.append("verb")
    if "aj" in raw or raw in {"a", "a?"}:
        poses.append("adjective")
    if "av" in raw or raw in {"r", "r?"}:
        poses.append("adverb")
    return poses


def scowl_base_surface(word_chunk: str) -> str | None:
    match = POS_TOKEN_RE.search(word_chunk)
    if not match:
        return None
    base = word_chunk[: match.start()].strip()
    return base if WORD_RE.fullmatch(base) else None


def scowl_base_text(word_chunk: str) -> str:
    match = POS_TOKEN_RE.search(word_chunk)
    if not match:
        return ""
    return word_chunk[: match.start()].strip()


def clean_inflection_surface(token: str) -> str | None:
    token = token.strip()
    if ":" in token:
        token = token.rsplit(":", 1)[1].strip()
    if not token or token in {"-", "?"}:
        return None
    token = token.replace("!", "").replace("~", "").replace("†", "")
    if any(ch in token for ch in ("'", " ", "-", ".", "/")):
        return None
    return token if WORD_RE.fullmatch(token) else None


def parse_scowl_line(line: str) -> tuple[str, str, re.Match[str]] | None:
    pos_match = POS_TOKEN_RE.search(line)
    if not pos_match:
        return None

    word_start = line.rfind(": ", 0, pos_match.start())
    if word_start == -1:
        return None
    word_start += 2

    inflection_sep = line.find(": ", pos_match.end())
    if inflection_sep == -1:
        word_chunk = line[word_start:]
        inflection_chunk = ""
    else:
        word_chunk = line[word_start:inflection_sep]
        inflection_chunk = line[inflection_sep + 2 :]
    return word_chunk, inflection_chunk, pos_match


def noun_like_inflection(base: str, surface: str) -> bool:
    if surface == base or surface.endswith(("ed", "ing")):
        return False
    return (
        surface in IRREGULAR_NOUN_PLURALS
        or surface.endswith(("s", "ves", "ies", "ae", "i", "a"))
    )


def verb_like_inflection(base: str, surface: str) -> bool:
    if surface == base:
        return False
    return (
        surface in IRREGULAR_VERB_FORMS
        or surface.endswith("ed")
        or surface.endswith("ing")
        or (surface.endswith("s") and not surface.endswith("ss"))
    )


def comparative_like_inflection(base: str, surface: str) -> bool:
    return surface != base and (surface in IRREGULAR_COMPARATIVE_FORMS or surface.endswith(("er", "est")))


def relation_for(part_of_speech: str, surface: str, index: int, count: int) -> str:
    if part_of_speech == "noun":
        return "plural of"
    if part_of_speech == "verb":
        if surface.endswith("ing"):
            return "present participle of"
        if surface.endswith("s") and not surface.endswith("ss"):
            return "third-person singular of"
        if surface in IRREGULAR_VERB_PAST_PARTICIPLES:
            return "past participle of"
        if surface in IRREGULAR_VERB_FORMS or surface.endswith("ed"):
            return "past tense of"
        return "inflected form of"
    if part_of_speech in {"adjective", "adverb"}:
        return "comparative of" if index == 0 else "superlative of" if index == 1 else "inflected form of"
    return "inflected form of"


def parse_scowl(
    scowl_path: Path,
    esdb_words: set[str],
) -> tuple[set[str], dict[str, list[Inflection]], set[str], dict[str, int]]:
    safe_words: set[str] = set()
    legacy_source_words: set[str] = set()
    inflections: dict[str, list[Inflection]] = collections.defaultdict(list)
    last_safe_base_by_pos: dict[str, str] = {}
    stats = collections.Counter()

    for raw in scowl_path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        stats["lines"] += 1

        line = line.split("#!", 1)[0].strip()
        excluded = scowl_is_excluded(line)
        legacy_supported = not excluded and scowl_has_legacy_evidence(line)
        parsed = parse_scowl_line(line)
        if not parsed:
            if legacy_supported and ": " in line:
                bare_word = line.rsplit(": ", 1)[-1].strip()
                if WORD_RE.fullmatch(bare_word):
                    legacy_source_words.add(bare_word)
            continue
        word_chunk, inflection_chunk, pos_match = parsed

        base_text = scowl_base_text(word_chunk)
        base = base_text if WORD_RE.fullmatch(base_text) else None
        legacy_base = base_text.lstrip("!@") if WORD_RE.fullmatch(base_text.lstrip("!@")) else None
        is_continuation = base_text == "-"
        if not base and not is_continuation:
            if legacy_base and legacy_supported:
                legacy_source_words.add(legacy_base)
            else:
                stats["invalid_base_rows"] += 1
                continue

        if base and base in esdb_words and not excluded:
            safe_words.add(base)
        elif base:
            stats["excluded_base_words"] += 1
        if base and legacy_supported:
            legacy_source_words.add(base)

        if excluded:
            stats["excluded_lines"] += 1
            continue

        raw_surfaces = [clean_inflection_surface(token) for token in re.split(r"[,|()]", inflection_chunk)]
        if legacy_supported:
            legacy_source_words.update(surface for surface in raw_surfaces if surface)

        poses = scowl_poses(pos_match.group(1))
        if not poses:
            continue

        if base and base in safe_words:
            for part_of_speech in poses:
                last_safe_base_by_pos[part_of_speech] = base

        surfaces = [surface for surface in raw_surfaces if surface and surface in esdb_words]
        if not surfaces:
            continue

        for part_of_speech in poses:
            inflection_base = base or (last_safe_base_by_pos.get(part_of_speech) if is_continuation else None)
            if not inflection_base or inflection_base not in safe_words:
                continue

            if part_of_speech == "noun":
                selected = [surface for surface in surfaces if noun_like_inflection(inflection_base, surface)]
            elif part_of_speech == "verb":
                selected = [surface for surface in surfaces if verb_like_inflection(inflection_base, surface)]
            elif part_of_speech in {"adjective", "adverb"}:
                selected = [surface for surface in surfaces if comparative_like_inflection(inflection_base, surface)]
            else:
                selected = []

            for index, surface in enumerate(selected):
                safe_words.add(surface)
                relation = relation_for(part_of_speech, surface, index, len(selected))
                inflections[surface].append(Inflection(inflection_base, part_of_speech, relation))

    stats["safe_words"] = len(safe_words)
    stats["legacy_source_words"] = len(legacy_source_words)
    stats["inflected_surfaces"] = len(inflections)
    return safe_words, inflections, legacy_source_words, dict(stats)


def load_wordnet_definitions(wordnet_zip: Path, safe_words: set[str]) -> dict[str, list[dict[str, str]]]:
    definitions: dict[str, list[dict[str, str]]] = collections.defaultdict(list)
    with zipfile.ZipFile(wordnet_zip) as archive:
        for name in archive.namelist():
            if name.startswith("entries-") or name == "frames.json":
                continue
            data = json.loads(archive.read(name))
            for synset in data.values():
                part_of_speech = PART_OF_SPEECH.get(synset.get("partOfSpeech"))
                synset_definitions = synset.get("definition") or []
                if not part_of_speech or not synset_definitions:
                    continue
                for member in synset.get("members") or []:
                    if member not in safe_words:
                        continue
                    for definition in synset_definitions[:2]:
                        add_definition(definitions, member, part_of_speech, definition)
    return definitions


def is_roman_numeral(word: str) -> bool:
    return bool(re.fullmatch(r"[ivxlcdm]+", word))


def legacy_current_allowed(word: str, entries: list[dict[str, str]], legacy_source_words: set[str]) -> bool:
    if not is_playable(word) or is_roman_numeral(word):
        return False
    if word in MANUAL_INCLUDE_CURRENT_WORDS or word in legacy_source_words:
        return True
    text = " ".join(entry.get("definition", "").lower() for entry in entries)
    if "of or relating to" in text or "a person from" in text or "relating to or characteristic" in text:
        return True
    return False


def load_current_definitions(
    current_json: Path,
    safe_words: set[str],
    legacy_source_words: set[str],
) -> tuple[dict[str, list[dict[str, str]]], set[str], set[str]]:
    raw = json.loads(current_json.read_text())
    current_playable = {word for word in raw if is_playable(word)}
    definitions: dict[str, list[dict[str, str]]] = collections.defaultdict(list)
    legacy_rescued: set[str] = set()
    for word, entries in raw.items():
        if word not in safe_words and not legacy_current_allowed(word, entries, legacy_source_words):
            continue
        if word not in safe_words:
            legacy_rescued.add(word)
        for entry in entries:
            add_definition(
                definitions,
                word,
                entry.get("part_of_speech", "word"),
                entry.get("definition", ""),
            )
    return definitions, current_playable, legacy_rescued


def load_manual_definitions() -> dict[str, list[dict[str, str]]]:
    definitions: dict[str, list[dict[str, str]]] = collections.defaultdict(list)
    for word, entries in MANUAL_DEFINITIONS.items():
        for entry in entries:
            add_definition(definitions, word, entry["part_of_speech"], entry["definition"])
    return definitions


def merge_definitions(
    current: dict[str, list[dict[str, str]]],
    wordnet: dict[str, list[dict[str, str]]],
    inflections: dict[str, list[Inflection]],
    manual: dict[str, list[dict[str, str]]],
    legacy_rescued: set[str],
) -> tuple[dict[str, list[dict[str, str]]], dict[str, set[str]]]:
    base_definitions: dict[str, list[dict[str, str]]] = collections.defaultdict(list)
    sources: dict[str, set[str]] = collections.defaultdict(set)

    for word, entries in current.items():
        for entry in entries:
            if add_definition(base_definitions, word, entry["part_of_speech"], entry["definition"]):
                sources[word].add("current")
                if word in legacy_rescued:
                    sources[word].add("legacy_rescue")

    for word, entries in wordnet.items():
        for entry in entries:
            if add_definition(base_definitions, word, entry["part_of_speech"], entry["definition"]):
                sources[word].add("wordnet")

    for word, entries in manual.items():
        for entry in entries:
            if add_definition(base_definitions, word, entry["part_of_speech"], entry["definition"]):
                sources[word].add("manual_definition")

    final: dict[str, list[dict[str, str]]] = collections.defaultdict(list)
    for word, entries in base_definitions.items():
        if not is_playable(word):
            continue
        for entry in select_definitions(entries):
            add_definition(final, word, entry["part_of_speech"], entry["definition"])

    for surface, candidates in inflections.items():
        if not is_playable(surface):
            continue
        seen = set()
        for candidate in candidates:
            key = (candidate.base, candidate.part_of_speech, candidate.relation)
            if key in seen or candidate.base not in base_definitions:
                continue
            seen.add(key)
            base_entries = [
                entry
                for entry in base_definitions[candidate.base]
                if entry["part_of_speech"] == candidate.part_of_speech
            ]
            if not base_entries:
                continue
            for entry in select_definitions(base_entries, 2):
                definition = f"{candidate.relation} {candidate.base.upper()}; {entry['definition']}"
                if add_definition(final, surface, candidate.part_of_speech, definition):
                    sources[surface].add("esdb_inflection")

    return {word: select_definitions(final[word]) for word in sorted(final)}, sources


def city_word_sets(words: set[str]) -> dict[str, set[str]]:
    return {city: {word for word in words if city in city_matches(word)} for city in CITIES}


def source_combos(final: dict[str, list[dict[str, str]]], sources: dict[str, set[str]]) -> dict[str, int]:
    combos = collections.Counter()
    for word in final:
        combo = "+".join(sorted(sources.get(word, {"unknown"})))
        combos[combo] += 1
    return dict(sorted(combos.items()))


def build_audit(
    final: dict[str, list[dict[str, str]]],
    current_playable: set[str],
    legacy_rescued: set[str],
    sources: dict[str, set[str]],
    esdb_words: set[str],
    esdb_default_words: set[str] | None,
    safe_words: set[str],
    wordnet_definitions: dict[str, list[dict[str, str]]],
    scowl_stats: dict[str, int],
) -> dict:
    final_words = set(final)
    old_by_city = city_word_sets(current_playable)
    new_by_city = city_word_sets(final_words)
    city_changes = []
    for city in CITIES:
        old = old_by_city[city]
        new = new_by_city[city]
        city_changes.append(
            {
                "city": city,
                "old_count": len(old),
                "new_count": len(new),
                "added_count": len(new - old),
                "removed_count": len(old - new),
                "added_words": sorted(new - old),
                "removed_words": sorted(old - new),
            }
        )

    return {
        "sources": SOURCE_NOTES,
        "filters": [
            "Only lowercase ASCII alphabetic words are accepted.",
            "Only words of length 4+ buildable from a configured city are shipped.",
            "ESDB/SCOWL entries tagged as abbreviations, uppercase/proper names, places, people, demonyms, surnames, towns, or roman numerals are excluded.",
            "Legacy playable words from the previous dictionary are retained when they have SCOWL legacy evidence, match accepted demonym/adjective patterns, or are in the project include list.",
            "Small manual definitions may add project-accepted words not covered by the downloaded sources.",
            "Open English WordNet multiword/underscore entries are not collapsed into fake single words.",
            "Inflected forms are accepted only when ESDB links them to a safe base word with definitions.",
        ],
        "summary": {
            "old_playable_words": len(current_playable),
            "new_playable_words": len(final_words),
            "added_words": len(final_words - current_playable),
            "removed_words": len(current_playable - final_words),
            "definition_entries": sum(len(entries) for entries in final.values()),
            "legacy_rescued_words": len(legacy_rescued & final_words),
            "esdb_large_lowercase_words": len(esdb_words),
            "esdb_large_playable_words": sum(1 for word in esdb_words if is_playable(word)),
            "esdb_default_lowercase_words": len(esdb_default_words) if esdb_default_words is not None else None,
            "esdb_default_playable_words": (
                sum(1 for word in esdb_default_words if is_playable(word)) if esdb_default_words is not None else None
            ),
            "safe_esdb_words": len(safe_words),
            "safe_esdb_playable_words": sum(1 for word in safe_words if is_playable(word)),
            "wordnet_safe_words_with_definitions": len(wordnet_definitions),
            "wordnet_safe_playable_words_with_definitions": sum(
                1 for word in wordnet_definitions if is_playable(word)
            ),
            "source_combinations": source_combos(final, sources),
            "scowl_stats": scowl_stats,
        },
        "city_changes": city_changes,
        "all_added_words": sorted(final_words - current_playable),
        "all_removed_words": sorted(current_playable - final_words),
    }


def write_gzip(path: Path, payload: bytes) -> None:
    with path.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as gz:
            gz.write(payload)


def markdown_list(words: list[str]) -> str:
    return ", ".join(f"`{word}`" for word in words) if words else "_none_"


def write_markdown_report(path: Path, audit: dict) -> None:
    summary = audit["summary"]
    lines = [
        "# Textropolis Dictionary Audit",
        "",
        "## Sources",
    ]
    for source in audit["sources"].values():
        lines.extend(
            [
                f"- {source['name']}: {source['role']}.",
                f"  - URL: {source['url']}",
                f"  - License: {source['license']}",
            ]
        )

    lines.extend(
        [
            "",
            "## Filters",
            *[f"- {item}" for item in audit["filters"]],
            "",
            "## Summary",
            "",
            "| Metric | Count |",
            "| --- | ---: |",
            f"| Old playable words | {summary['old_playable_words']} |",
            f"| New playable words | {summary['new_playable_words']} |",
            f"| Added words | {summary['added_words']} |",
            f"| Removed words | {summary['removed_words']} |",
            f"| Definition entries | {summary['definition_entries']} |",
            f"| Legacy words rescued | {summary['legacy_rescued_words']} |",
            f"| ESDB large lowercase words | {summary['esdb_large_lowercase_words']} |",
            f"| ESDB large playable words | {summary['esdb_large_playable_words']} |",
            f"| ESDB default lowercase words | {summary['esdb_default_lowercase_words']} |",
            f"| ESDB default playable words | {summary['esdb_default_playable_words']} |",
            f"| Safe ESDB words after category filtering | {summary['safe_esdb_words']} |",
            f"| Safe ESDB playable words after category filtering | {summary['safe_esdb_playable_words']} |",
            f"| WordNet safe words with definitions | {summary['wordnet_safe_words_with_definitions']} |",
            f"| WordNet safe playable words with definitions | {summary['wordnet_safe_playable_words_with_definitions']} |",
            "",
            "## Source Mix",
            "",
            "| Sources | Words |",
            "| --- | ---: |",
        ]
    )
    for combo, count in summary["source_combinations"].items():
        lines.append(f"| `{combo}` | {count} |")

    lines.extend(
        [
            "",
            "## City Counts",
            "",
            "| City | Old | New | Added | Removed |",
            "| --- | ---: | ---: | ---: | ---: |",
        ]
    )
    for city in audit["city_changes"]:
        lines.append(
            f"| {city['city']} | {city['old_count']} | {city['new_count']} | "
            f"{city['added_count']} | {city['removed_count']} |"
        )

    lines.extend(["", "## City Word Changes"])
    for city in audit["city_changes"]:
        lines.extend(
            [
                "",
                f"### {city['city']}",
                "",
                f"- Added ({city['added_count']}): {markdown_list(city['added_words'])}",
                f"- Removed ({city['removed_count']}): {markdown_list(city['removed_words'])}",
            ]
        )

    path.write_text("\n".join(lines) + "\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--current-json", type=Path, default=Path("textropolis-dictionary.json"))
    parser.add_argument("--wordnet-zip", type=Path, required=True)
    parser.add_argument("--esdb-large-wordlist", type=Path, required=True)
    parser.add_argument("--esdb-default-wordlist", type=Path)
    parser.add_argument("--scowl", type=Path, required=True)
    parser.add_argument("--output-json", type=Path, default=Path("textropolis-dictionary.json"))
    parser.add_argument("--output-gzip", type=Path, default=Path("textropolis-dictionary.json.gz"))
    parser.add_argument("--audit-json", type=Path, default=Path("textropolis-dictionary-audit.json"))
    parser.add_argument("--audit-md", type=Path, default=Path("textropolis-dictionary-audit.md"))
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    esdb_words = lowercase_words(args.esdb_large_wordlist)
    esdb_default_words = lowercase_words(args.esdb_default_wordlist) if args.esdb_default_wordlist else None
    safe_words, inflections, legacy_source_words, scowl_stats = parse_scowl(args.scowl, esdb_words)
    safe_words.update(MANUAL_INCLUDE_WORDS)
    current_definitions, current_playable, legacy_rescued = load_current_definitions(
        args.current_json,
        safe_words,
        legacy_source_words,
    )
    wordnet_definitions = load_wordnet_definitions(args.wordnet_zip, safe_words)
    manual_definitions = load_manual_definitions()
    final, sources = merge_definitions(
        current_definitions,
        wordnet_definitions,
        inflections,
        manual_definitions,
        legacy_rescued,
    )
    audit = build_audit(
        final,
        current_playable,
        legacy_rescued,
        sources,
        esdb_words,
        esdb_default_words,
        safe_words,
        wordnet_definitions,
        scowl_stats,
    )

    payload = (json.dumps(final, indent=2, sort_keys=True) + "\n").encode()
    args.output_json.write_bytes(payload)
    write_gzip(args.output_gzip, payload)
    args.audit_json.write_text(json.dumps(audit, indent=2, sort_keys=True) + "\n")
    write_markdown_report(args.audit_md, audit)

    summary = audit["summary"]
    print(
        f"wrote {args.output_json} and {args.output_gzip}: "
        f"{summary['new_playable_words']} words, {summary['definition_entries']} definitions"
    )
    print(f"added {summary['added_words']} words, removed {summary['removed_words']} words")


if __name__ == "__main__":
    main()
