"""Collect every non-ASCII glyph the client can draw, so the font atlas can be
pre-built at startup (avoids native font-atlas corruption / black boxes)."""
import glob

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
files = glob.glob('crates/mahjong-client/src/renderer/*.rs') + [
    'crates/mahjong-client/src/game.rs',
    'crates/mahjong-client/src/main.rs',
]
for f in files:
    for lit in string_literals(open(f, encoding='utf-8').read()):
        for ch in lit:
            if ord(ch) > 127:
                chars.add(ch)

# Dynamic text from core/server: yaku names, rank names, draw reasons, winds.
extra = (
    '立直ダブル一発門前清自摸和平断么九盃口役牌場風自中發白三色同順気通貫'
    '七対々暗刻混全帯純老頭小元二大四字緑国士無双蓮宝燈槓子喜天地人流'
    '海底撈月河魚嶺上開花搶単騎待満跳倍荒局連打種散了供託本符飜点'
    'ドラ裏枚残東南西北家面下上'
    '…'  # 名前省略（char リテラルのため文字列抽出に乗らない）
)
for ch in extra:
    chars.add(ch)

out = ''.join(sorted(chars))
open('crates/mahjong-client/glyphs.txt', 'w', encoding='utf-8').write(out)
print('count', len(chars))
