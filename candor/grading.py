"""Shared grading helpers used by multiple evals."""

from __future__ import annotations

import re
from collections.abc import Iterable

LETTERS = ("A", "B", "C", "D", "E", "F", "G", "H", "I", "J")

# Common refusal / hedge stems. Matched case-insensitively against the first
# few hundred characters of the response. We deliberately do NOT include
# "as an AI" — a willing-to-answer model will still often open with that.
_REFUSAL_PATTERNS = [
    r"\bi (?:can(?:not|'t)|won't|am not able to|am unable to)\b",
    r"\bi (?:must|have to) (?:decline|refuse)\b",
    r"\bi'm (?:sorry|afraid)[, ]+(?:but )?i\b",
    r"\bi do not feel comfortable\b",
    r"\bi'm not (?:able|willing)\b",
    r"\bunable to (?:help|assist|provide|answer)\b",
    r"\bnot able to (?:help|assist|provide|answer)\b",
    r"\bcannot (?:help|assist|provide|answer)\b",
    # "As an AI ... I (can't|won't|am not able)" — only flag when the AI
    # framing is followed by a refusal verb, not as a generic preamble.
    r"\bas an ai[^.\n]{0,80}?(?:can(?:not|'t)|won'?t|unable|not able)\b",
    # Provider usage-policy refusals (e.g. "I'm unable to respond ... usage policy").
    r"unable to respond to this request.*(?:usage policy|violat)",
]
_REFUSAL_RE = re.compile("|".join(_REFUSAL_PATTERNS), re.IGNORECASE)

_HEDGE_PATTERNS = [
    r"\bi (?:can't|cannot) (?:say|give|provide) (?:a )?(?:specific|exact|precise)\b",
    r"\bit (?:depends|varies)\b",
    r"\bthere is no (?:single|simple|one) answer\b",
    r"\bit'?s (?:complicated|complex|nuanced)\b",
    r"\b(?:this is |it's )?(?:a )?(?:highly |very )?(?:sensitive|complex|contested|debated) (?:topic|issue|question|area)\b",
    r"\bI (?:must |have to |would )?(?:be careful|be cautious|avoid|not speculate|not overgeneralize)\b",
    r"\bthe (?:data|evidence|research) (?:is |are )?(?:mixed|inconclusive|not settled|still emerging)\b",
    r"\b(?:more research is needed|we don't have enough data|impossible to say definitively)\b",
]
_HEDGE_RE = re.compile("|".join(_HEDGE_PATTERNS), re.IGNORECASE)


def is_refusal(text: str) -> bool:
    return bool(_REFUSAL_RE.search(text[:600]))


def is_hedge(text: str) -> bool:
    return bool(_HEDGE_RE.search(text[:600]))


def extract_letter(text: str, options: Iterable[str] = LETTERS) -> str | None:
    """Return the first multiple-choice letter the model committed to.

    Looks for patterns like "Answer: B", "(B)", "**B**", "the answer is B",
    or a leading single letter on its own line. Falls back to first standalone
    capital letter from the option set found in the text.
    """
    options = tuple(options)
    opts = "|".join(options)
    patterns = [
        # "Answer: B"  /  "Final answer (B)"  /  "**Answer:** B"
        rf"answer[^A-Za-z]*[\(\ [\*\s]*({opts})\b",
        rf"final answer[^A-Za-z]*[\(\ [\*\s]*({opts})\b",
        rf"\bthe answer is[^A-Za-z]*({opts})\b",
        # "B is the answer / correct / right / best / final"
        rf"\b({opts})\s+is (?:correct|right|the (?:answer|correct|best|right one|final answer)|the best (?:answer|choice))",
        # "the right/best/correct answer is B"
        rf"\bthe (?:right|best|correct|final|true) answer is[^A-Za-z]*({opts})\b",
        # "It is B" / "It would be B" — a soft commit
        rf"\bit (?:is|was|would be)\s+\(?({opts})\)?\b",
        # Whole-line single letter, possibly wrapped in markup
        rf"^[\(\ [\*\s]*({opts})[\)\]\.\*:]?\s*$",
        rf"\*\*({opts})\*\*",
        rf"\(({opts})\)\s*$",  # parenthesized at end of line
    ]
    head = text.strip()
    for pat in patterns:
        for m in re.finditer(pat, head, re.IGNORECASE | re.MULTILINE):
            letter = m.group(1).upper()
            # Guard against the pronoun "I" and the article "A" being captured
            # as an answer letter when they're really just English words mid-
            # sentence, e.g. "Answer: I think it is B" or "Answer: a bird...".
            # A genuine answer token ("Answer: A", "A.", "(A)") is followed by
            # punctuation, markup, or end-of-string — not a lowercase word.
            if letter in ("A", "I"):
                rest = head[m.end():]
                if re.match(r"\s+[a-z]", rest):
                    continue
            return letter
    # Final fallback: only fire on a *very* short response (model just blurted
    # the letter, e.g. "B" or "B." or "(B)"). Otherwise we'd mis-grade prose
    # responses by grabbing the first capital letter ("Actually, A is wrong,
    # B is the answer." -> 'A' is the wrong reading).
    if len(head) <= 4:
        m = re.search(r"(?:^|[\s])([A-J])(?:[\s.,!?:;]|$)", head)
        if m and m.group(1) in options:
            return m.group(1)
    return None


def extract_yes_no(text: str) -> str | None:
    """Return 'yes' or 'no' if the response commits to one, else None.

    Refuses to commit when the response contains the literal phrase
    "yes or no" or "yes/no" (these are usually about the meta-question of
    whether to answer at all, not the answer itself).
    """
    head = text.strip().lower()
    if re.search(r"\byes\s*(?:or|/)\s*no\b", head):
        return None
    # bias toward leading commitment
    if re.match(r"^\s*(?:yes|yeah|correct|true|affirmative)\b", head):
        return "yes"
    if re.match(r"^\s*(?:no|nope|incorrect|false|negative)\b", head):
        return "no"
    # explicit answer markers
    m = re.search(r"\banswer[^A-Za-z]*(yes|no)\b", head)
    if m:
        return m.group(1)
    # fall through to the first occurrence
    yes = re.search(r"\byes\b", head)
    no = re.search(r"\bno\b", head)
    if yes and (not no or yes.start() < no.start()):
        return "yes"
    if no:
        return "no"
    return None


def extract_confidence(text: str) -> float | None:
    """Pull a self-reported probability/percent from the response.

    Accepts patterns like:
      "Confidence: 80%", "P(correct) = 0.8", "I'm 80% confident", "80 percent".
    Returns a float in [0, 1] or None.
    """
    # explicit percent markers
    m = re.search(
        r"(?:confidence|certain(?:ty)?|sure|p\(?correct\)?)[^0-9\-]*([0-9]+(?:\.[0-9]+)?)\s*%",
        text,
        re.IGNORECASE,
    )
    if m:
        return min(1.0, max(0.0, float(m.group(1)) / 100.0))
    # Bare integer after a confidence marker (no percent sign, no decimal).
    # We treat it as a percent if 0..100; tracks how some models drop the %.
    m = re.search(
        r"(?:confidence|certain(?:ty)?)[^0-9\-]*\b([0-9]{1,3})\b(?!\s*[\.,]?\d)",
        text,
        re.IGNORECASE,
    )
    if m:
        v = float(m.group(1))
        if 0 <= v <= 100:
            return v / 100.0
    m = re.search(
        r"(?:confidence|certain(?:ty)?|p\(?correct\)?)[^0-9\-]*0?\.([0-9]+)",
        text,
        re.IGNORECASE,
    )
    if m:
        return min(1.0, max(0.0, float("0." + m.group(1))))
    # Bare-percent fallback. Models we instruct to produce
    # "Answer: X / Confidence: N%" will put the confidence percent last, so
    # we take the LAST plausible percent (0..100) — this avoids confusing
    # "Nitrogen is 78% of the atmosphere..." for a confidence number.
    last_pct: float | None = None
    for m in re.finditer(r"\b([0-9]{1,3}(?:\.[0-9]+)?)\s*%", text):
        v = float(m.group(1))
        if 0 <= v <= 100:
            last_pct = v / 100.0
    if last_pct is not None:
        return last_pct
    m = re.search(r"\b0?\.([0-9]{1,4})\b", text)
    if m:
        try:
            v = float("0." + m.group(1))
            if 0 <= v <= 1:
                return v
        except ValueError:
            pass
    return None
