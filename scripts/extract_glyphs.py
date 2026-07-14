"""Collect every non-ASCII glyph the client can draw.

The generated atlas avoids native font corruption and black boxes.
"""

from pathlib import Path


def string_literals(src):
    """Yield the contents of double-quoted string literals (rough, escape-aware)."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c == '"':
            buf, i = [], i + 1
            while i < n and src[i] != '"':
                if src[i] == '\\' and i + 1 < n:
                    buf.append(src[i + 1])
                    i += 2
                else:
                    buf.append(src[i])
                    i += 1
            out.append(''.join(buf))
        i += 1
    return out


chars = set()
source_root = Path('crates/mahjong-client/src')
files = [path for path in source_root.rglob('*.rs') if path.name != 'tests.rs']
files.extend(
    Path(path)
    for path in (
        'crates/mahjong-core/src/scoring/score.rs',
        'crates/mahjong-core/src/winning_hand/name.rs',
    )
)
for path in files:
    for lit in string_literals(path.read_text(encoding='utf-8')):
        for ch in lit:
            if ord(ch) > 127:
                chars.add(ch)

# These are built from character literals or a dependency API, so the string
# scanner cannot discover them from client source.
chars.update('…東南西北')

out = ''.join(sorted(chars))
Path('crates/mahjong-client/glyphs.txt').write_text(out, encoding='utf-8')
print('count', len(chars))
