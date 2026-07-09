"""Convert Project Gutenberg plain texts to the web emulator's book markup.

Markup: '# ' chapter heading, '~ ' centered verse line, blank-line
paragraphs. Normalizes typography the 1-bit Literata bitmaps may lack.
"""

import re
import sys

OUT = sys.argv[2] if len(sys.argv) > 2 else "."
SRC = sys.argv[1] if len(sys.argv) > 1 else "."

ROMAN = {
    "I": "One", "II": "Two", "III": "Three", "IV": "Four", "V": "Five",
    "VI": "Six", "VII": "Seven", "VIII": "Eight", "IX": "Nine", "X": "Ten",
    "XI": "Eleven", "XII": "Twelve",
}


SMALL_WORDS = {"a", "an", "and", "of", "the", "in", "on", "his", "her", "to", "with", "at", "by", "for"}


def title_case(text: str) -> str:
    words = text.lower().split()
    out = []
    for index, word in enumerate(words):
        if index > 0 and word in SMALL_WORDS:
            out.append(word)
        else:
            out.append(word[:1].upper() + word[1:])
    return " ".join(out)


def normalize(text: str) -> str:
    for a, b in [
        ("“", '"'), ("”", '"'), ("‘", "'"), ("’", "'"),
        ("Æ", "Ae"), ("æ", "ae"), ("Œ", "Oe"), ("œ", "oe"),
        ("…", "..."), ("–", "-"), (" ", " "),
        ("—", "--"),
        ("ê", "e"), ("é", "e"), ("è", "e"), ("ë", "e"), ("â", "a"),
        ("à", "a"), ("ô", "o"), ("î", "i"), ("ç", "c"), ("ï", "i"),
        ("ü", "u"), ("ñ", "n"), ("«", '"'), ("»", '"'),
        # This Pegana edition renders the macron'd river names Eimes,
        # Zanes, Segastrion with stray glyphs; fold them to plain vowels.
        ("Î", "e"), ("‰", "a"), ("·", "a"),
        ("_", ""),  # PG italics markers
    ]:
        text = text.replace(a, b)
    return text


def body(path: str) -> list[str]:
    raw = open(path, encoding="utf-8").read()
    raw = raw.split("***", 2)[2]          # after START sentinel
    raw = raw.rsplit("*** END", 1)[0]
    return normalize(raw).splitlines()


def flush(par: list[str], out: list[str]) -> None:
    if par:
        out.append(" ".join(word for word in " ".join(par).split()))
        out.append("")
        par.clear()


def emit_stream(lines: list[str], out: list[str], par: list[str]) -> None:
    """Paragraphs + indented lines as centered verse."""
    for line in lines:
        if not line.strip():
            flush(par, out)
        elif re.match(r"^\s{4,}\S", line):
            flush(par, out)
            out.append("~ " + line.strip())
        else:
            par.append(line.strip())
    flush(par, out)


def alice() -> None:
    lines = body(f"{SRC}/alice.txt")
    out: list[str] = []
    par: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        match = re.match(r"^CHAPTER ([IVX]+)\.\s*$", line)
        if match:
            flush(par, out)
            title = lines[i + 1].strip()
            out.append(f"# {ROMAN[match.group(1)]}. {title}")
            out.append("")
            i += 2
            continue
        if not line.strip():
            flush(par, out)
        elif re.match(r"^\s{4,}\S", line):
            flush(par, out)
            out.append("~ " + line.strip())
        else:
            par.append(line.strip())
        i += 1
    flush(par, out)
    text = "\n".join(out)
    text = text.split("# One.", 1)[1]
    open(f"{OUT}/alice.txt", "w").write("# One." + text.rstrip() + "\n")


def carol() -> None:
    lines = body(f"{SRC}/carol.txt")
    out: list[str] = []
    par: list[str] = []
    for line in lines:
        match = re.match(r"^STAVE ([IVX]+):\s*(.+)$", line)
        if match:
            flush(par, out)
            out.append(f"# Stave {ROMAN[match.group(1)]}: {title_case(match.group(2))}")
            out.append("")
        elif not line.strip():
            flush(par, out)
        elif re.match(r"^\s{6,}\S", line):
            flush(par, out)
            out.append("~ " + line.strip())
        else:
            par.append(line.strip())
    flush(par, out)
    text = "\n".join(out)
    text = "# Stave" + text.split("# Stave", 1)[1]
    open(f"{OUT}/carol.txt", "w").write(text.rstrip() + "\n")


def aesop(count: int = 40) -> None:
    lines = body(f"{SRC}/aesop.txt")
    # Table-of-contents titles appear early, one per line, indented one space.
    toc: list[str] = []
    for line in lines[:400]:
        match = re.match(r"^ (The .+|[A-Z][a-z].+)$", line)
        if match and len(match.group(1)) < 60:
            toc.append(match.group(1).strip().lower())
    titles = set(toc)

    out: list[str] = []
    par: list[str] = []
    fables = 0
    started = False
    for index, line in enumerate(lines):
        stripped = line.strip()
        is_title = (
            stripped.lower() in titles
            and index > 700                      # past the TOC and front matter
            and index + 1 < len(lines)
            and not lines[index + 1].strip()     # blank after a heading
            and not line.startswith(" ")
        )
        if is_title:
            if fables >= count:
                break
            flush(par, out)
            out.append(f"# {title_case(stripped)}")
            out.append("")
            fables += 1
            started = True
        elif started:
            if not stripped:
                flush(par, out)
            else:
                par.append(stripped)
    flush(par, out)
    open(f"{OUT}/aesop.txt", "w").write("\n".join(out).rstrip() + "\n")
    print(f"aesop: {fables} fables")


def cap_first_alpha(word: str) -> str:
    """Capitalize the first letter, and any letter after a hyphen, so
    'anglo-french' -> 'Anglo-French' and '"thunder' -> '"Thunder'."""
    out = []
    cap_next = True
    for ch in word:
        if cap_next and ch.isalpha():
            out.append(ch.upper())
            cap_next = False
        else:
            out.append(ch)
        if ch == "-":
            cap_next = True
    return "".join(out)


def head_case(text: str) -> str:
    """Title-case a heading, capitalizing through leading quotes/punctuation."""
    small = SMALL_WORDS | {"or", "nor", "but", "as"}
    words = text.lower().split()
    out = []
    for index, word in enumerate(words):
        if 0 < index < len(words) - 1 and word in small:
            out.append(word)
        else:
            out.append(cap_first_alpha(word))
    return " ".join(out)


def emit_heading(out: list[str], title: str) -> None:
    title = title.rstrip(". ")
    if out and out[-1] != "":
        out.append("")
    out.append(f"# {title}")
    out.append("")


def marker_book(name: str, marker: "re.Pattern", book_re) -> None:
    """A book whose chapters are a numeral line followed by a title line.

    `book_re` (optional) matches a 'BOOK ONE' part divider whose subtitle is
    the next line; used by The War of the Worlds' two-part structure.
    """
    lines = body(f"{SRC}/{name}.txt")
    out: list[str] = []
    par: list[str] = []
    started = False
    i = 0
    while i < len(lines):
        line = lines[i]
        if book_re and book_re.match(line.strip()):
            sub = lines[i + 1].strip() if i + 1 < len(lines) else ""
            flush(par, out)
            emit_heading(out, f"{head_case(line.strip())}: {head_case(sub)}"
                         if sub else head_case(line.strip()))
            started = True
            i += 2
            continue
        if marker.match(line):
            j = i + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            title = head_case(lines[j].strip()) if j < len(lines) else ""
            flush(par, out)
            emit_heading(out, title)
            started = True
            i = j + 1
            continue
        if not started:
            i += 1
            continue
        if not line.strip():
            flush(par, out)
        else:
            par.append(line.strip())
        i += 1
    flush(par, out)
    text = "\n".join(out)
    text = "# " + text.split("# ", 1)[1]      # drop front matter/TOC
    open(f"{OUT}/{name}.txt", "w").write(text.rstrip() + "\n")


def pegana() -> None:
    """All-caps standalone lines are headings; consecutive ones merge."""
    lines = body(f"{SRC}/pegana.txt")
    caps = re.compile(r"^[A-Z][A-Z'’ .-]*[A-Z.]$")
    out: list[str] = []
    par: list[str] = []
    started = False
    i = next(k for k, l in enumerate(lines) if l.strip() == "PREFACE")
    while i < len(lines):
        stripped = lines[i].strip()
        is_head = (
            caps.match(stripped) is not None
            and 2 < len(stripped) < 60
            and (i == 0 or not lines[i - 1].strip())
        )
        if is_head:
            parts = [stripped]
            j = i + 1
            while j < len(lines) and lines[j].strip() and caps.match(lines[j].strip()):
                parts.append(lines[j].strip())
                j += 1
            flush(par, out)
            emit_heading(out, head_case(" ".join(parts)))
            started = True
            i = j
            continue
        if not started:
            i += 1
            continue
        if not stripped:
            flush(par, out)
        else:
            par.append(stripped)
        i += 1
    flush(par, out)
    text = "\n".join(out)
    text = "# " + text.split("# ", 1)[1]
    open(f"{OUT}/pegana.txt", "w").write(text.rstrip() + "\n")


def lastmen() -> None:
    """Stapledon: indented `<roman> Title` chapters and `N. TITLE` sections.

    Underscores are already stripped by normalize, so chapter headings read
    `      I Balkan Europe`. Starts at the in-universe Introduction, skipping
    the Contents and authorial Preface like the other books drop front matter.
    """
    lines = body(f"{SRC}/lastmen.txt")
    chap = re.compile(r"^ {4,}([IVXL]+) ([A-Z][A-Za-z].*)$")
    sect = re.compile(r"^ {4,}\d+\. ([A-Z].*)$")
    spaced = re.compile(r"^ {4,}([A-Z] )+[A-Z],?.*$")     # 'T H E   C H R O N I C L E'
    intro = re.compile(r"^ {4,}Introduction\s*$")
    out: list[str] = []
    par: list[str] = []
    i = next(k for k, l in enumerate(lines) if intro.match(l))
    emit_heading(out, "Introduction")
    i += 1
    while i < len(lines):
        line = lines[i]
        if intro.match(line) or spaced.match(line):
            i += 1
            continue
        m = chap.match(line)
        if m:
            flush(par, out)
            emit_heading(out, head_case(m.group(2)))
            i += 1
            continue
        m = sect.match(line)
        if m:
            flush(par, out)
            emit_heading(out, head_case(m.group(1)))
            i += 1
            continue
        stripped = line.strip()
        if not stripped:
            flush(par, out)
        elif stripped == "By One of the Last Men":
            pass                                          # intro subtitle, drop
        else:
            par.append(stripped)
        i += 1
    flush(par, out)
    text = "\n".join(out)
    text = "# " + text.split("# ", 1)[1]
    open(f"{OUT}/lastmen.txt", "w").write(text.rstrip() + "\n")


ROMAN_LINE = re.compile(r"^\s*[IVXLC]+\.\s*$")            # ' I.'  'II.'
CHAPTER_LINE = re.compile(r"^CHAPTER\s+[IVXLC]+\s*$")     # 'CHAPTER I'
BOOK_LINE = re.compile(r"^BOOK\s+(ONE|TWO|THREE)$")

alice()
carol()
aesop()
pegana()
marker_book("timemachine", ROMAN_LINE, None)
marker_book("warworlds", ROMAN_LINE, BOOK_LINE)
marker_book("mars", CHAPTER_LINE, None)
lastmen()
for name in ["alice", "carol", "aesop", "pegana", "timemachine", "warworlds", "mars", "lastmen"]:
    text = open(f"{OUT}/{name}.txt").read()
    chapters = text.count("\n# ") + text.startswith("# ")
    print(name, len(text), "bytes,", chapters, "chapters")
