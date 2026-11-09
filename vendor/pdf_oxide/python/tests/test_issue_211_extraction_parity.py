"""Issue #211 regression suite: reading-order parity on three pdfplumber
fixtures that drove the #457 rewire.

Every test is a standing regression guard — the bugs they describe were
fixed in 0.3.42. If one starts failing in the future, the canonical
reading-order pipeline (struct tree on tagged PDFs, geometric fallback
otherwise) has drifted again.

Fixtures live in the external pdf_oxide_tests corpus
(``~/projects/pdf_oxide_tests/pdfs_issue_regression/``). Tests skip
gracefully when the corpus is not present.
"""

from __future__ import annotations

from pathlib import Path

import pytest

import pdf_oxide


_FIXTURE_DIR = Path.home() / "projects" / "pdf_oxide_tests" / "pdfs_issue_regression"


def _load(name: str) -> pdf_oxide.PdfDocument:
    path = _FIXTURE_DIR / name
    if not path.exists():
        pytest.skip(f"fixture not found: {path}")
    return pdf_oxide.PdfDocument.from_bytes(path.read_bytes())


def _assert_monotonic_line_y(lines) -> None:
    prev_y = float("inf")
    for i, ln in enumerate(lines):
        y = ln.bbox[1]
        assert y <= prev_y + 0.5, (
            f"lines not monotonic at index {i}: y={y} after prev_y={prev_y}, text={ln.text!r}"
        )
        prev_y = y


# ── PDF #1: pdf_structure.pdf — Lorem-ipsum demo ────────────────────────────


def test_211_pdf_structure_first_words_in_order() -> None:
    doc = _load("issue_211_pdf_structure.pdf")
    words = doc.extract_words(0)
    assert words, "must extract at least one word"
    assert words[0].text == "Titre"
    assert words[1].text == "du"
    assert words[2].text == "document"


def test_211_pdf_structure_lines_monotonic_y() -> None:
    doc = _load("issue_211_pdf_structure.pdf")
    lines = doc.extract_text_lines(0)
    assert len(lines) >= 20, "should extract ~22 lines"
    _assert_monotonic_line_y(lines)


# ── PDF #2: municipal_minutes — centered title above body ───────────────────


def test_211_municipal_minutes_first_word_is_comite() -> None:
    doc = _load("issue_211_municipal_minutes.pdf")
    words = doc.extract_words(0)
    head = [w.text for w in words[:8]]
    assert words[0].text == "COMITÉ", f"first word should be 'COMITÉ'; got prefix {head!r}"


def test_211_municipal_minutes_first_line_is_title() -> None:
    doc = _load("issue_211_municipal_minutes.pdf")
    lines = doc.extract_text_lines(0)
    head = [ln.text for ln in lines[:5]]
    assert lines[0].text == "COMITÉ DE DÉMOLITION", (
        f"first line should be the title; got prefix {head!r}"
    )


def test_211_municipal_minutes_lines_monotonic_y() -> None:
    doc = _load("issue_211_municipal_minutes.pdf")
    _assert_monotonic_line_y(doc.extract_text_lines(0))


def test_211_municipal_minutes_spans_contain_title() -> None:
    """extract_spans returns the title in correct order — guard."""
    doc = _load("issue_211_municipal_minutes.pdf")
    spans = doc.extract_spans(0)
    joined = " ".join(s.text for s in spans)
    assert "COMITÉ DE DÉMOLITION" in joined
    assert "PROCÈS-VERBAL" in joined
    title_pos = joined.find("COMITÉ DE DÉMOLITION")
    body_pos = joined.find("Séance publique")
    assert title_pos < body_pos, "title must precede body in extract_spans output"


def test_211_municipal_minutes_words_match_span_order() -> None:
    doc = _load("issue_211_municipal_minutes.pdf")
    words = [w.text for w in doc.extract_words(0)]
    assert "COMITÉ" in words and "Séance" in words
    comite_idx = words.index("COMITÉ")
    seance_idx = words.index("Séance")
    assert comite_idx < seance_idx, (
        f"COMITÉ (title, y≈871) must precede Séance (body, y≈827); "
        f"got COMITÉ@{comite_idx} Séance@{seance_idx}"
    )


# ── PDF #3: government_form — form-style label/value layout ─────────────────


def test_211_government_form_prose_line_not_split() -> None:
    """The full prose sentence — split across two same-y spans in the source —
    must end up on a single extracted line. We tolerate any whitespace
    artifact between the prefix and the continuation: the user-visible bug
    from #211 was the *split*, not the joiner-whitespace shape."""
    doc = _load("issue_211_government_form.pdf")
    lines = doc.extract_text_lines(0)
    prefix = "Reports submitted to the Division of Safety and Permanence"
    suffix = "that do not include all of the required information"
    prose_line = next((ln for ln in lines if prefix in ln.text), None)
    assert prose_line is not None, "no line contains the prose prefix"
    assert suffix in prose_line.text, (
        f"prefix and continuation must be on the same line; got:\n{prose_line.text}"
    )


def test_211_government_form_lines_monotonic_y() -> None:
    doc = _load("issue_211_government_form.pdf")
    _assert_monotonic_line_y(doc.extract_text_lines(0))


# ── pdfplumber word-count parity (within ±5%) ───────────────────────────────


def test_211_extract_words_count_within_5pct_of_pdfplumber() -> None:
    pdfplumber = pytest.importorskip("pdfplumber")
    counts = []
    for name in (
        "issue_211_pdf_structure.pdf",
        "issue_211_municipal_minutes.pdf",
        "issue_211_government_form.pdf",
    ):
        path = _FIXTURE_DIR / name
        if not path.exists():
            pytest.skip(f"fixture not found: {path}")
        ours = len(pdf_oxide.PdfDocument.from_bytes(path.read_bytes()).extract_words(0))
        with pdfplumber.open(str(path)) as pp:
            theirs = len(pp.pages[0].extract_words())
        delta = abs(ours - theirs) / max(theirs, 1)
        counts.append((name, ours, theirs, delta))
    bad = [c for c in counts if c[3] > 0.05]
    assert not bad, "word counts diverge from pdfplumber by >5%: " + str(bad)
