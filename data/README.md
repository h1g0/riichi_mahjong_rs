# Machine-readable data

Data files published alongside the code so that tooling outside this repository
— and outside Rust — can consume them without re-typing the tables.

## `yaku.json`

The yaku name mapping from [`docs/glossary.md`](../docs/glossary.md), in three
naming systems at once:

- the **WRC Rules 2025** official English name,
- the **Japanese** name,
- the **mjai** romaji label (from Tenhou's yaku ids).

Each entry is keyed by `kind`, the `Kind` enum variant in
[`winning_hand::name`](../crates/mahjong-core/src/winning_hand/name.rs), so a row
stays traceable to the code that produces it.

### Direction of truth

`name.rs` is the source of truth; this file mirrors it. Generating the Rust from
the JSON was considered and rejected: the enum also fixes the display order of
equal-han yaku and carries per-variant documentation, and a build script would
have to be reproduced for the WASM target. Instead the two are pinned together
by tests that fail if either side changes alone —
`winning_hand::yaku_data_tests` in `mahjong-core` for the English and Japanese
names, and `yaku_tests` in `mahjong-mjai` for the mjai labels.

Editing this file alone will not change the game's output. Change `name.rs`
first, then mirror it here.

### Schema

Top level:

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | integer | Bumped on any incompatible change to the shape below. |
| `source_of_truth` | string | Path to the Rust module this mirrors. |
| `english_names` | object | Provenance of the English column: `source` and `url`. |
| `open_suffix` | object | The `en` / `ja` suffix appended to `open_en` / `open_ja`. |
| `yaku` | array | One entry per yaku, in the enum's declaration order. |

Each `yaku` entry:

| Field | Type | Meaning |
|---|---|---|
| `kind` | string | `Kind` variant name. Unique; the key of the table. |
| `en` | string | WRC Rules 2025 English name. |
| `ja` | string | Japanese name. |
| `romaji` | string | WRC romanized reading, with macrons. |
| `mjai` | string | mjai label. **Many-to-one** — see below. |
| `closed_only` | boolean | Whether the yaku requires a closed hand. |
| `open_en` | string \| null | English name on an open hand, or `null` when it does not change. |
| `open_ja` | string \| null | Japanese name on an open hand, or `null` when it does not change. |
| `value` | object | What the yaku is worth. |
| `double_yakuman_option` | boolean | Present and `true` only when the yaku counts double under the optional double-yakuman rule (`Settings::double_yakuman`, on by default). |

`value` has a `unit` discriminant:

- `{"unit": "han", "closed": <int>, "open": <int> \| null}` — han, before dora.
- `{"unit": "yakuman", "closed": <int>, "open": <int> \| null}` — yakuman
  multiples, before `double_yakuman_option`.
- `{"unit": "mangan"}` — a fixed mangan, and neither han nor yakuman. Only
  Nagashi Mangan.

`open` is `null` exactly when `closed_only` is true. `open_en` / `open_ja` are
non-null exactly for the yaku that lose han when open, and are always the plain
name plus `open_suffix`.

### Caveats

- **`mjai` is not a key.** mjai collapses distinct yaku onto one label: all
  three dragon Value Honours are `sangenpai`, both four-concealed-triplet forms
  are `suanko`, both nine-gates forms are `churenpoton`, and both thirteen-
  orphans forms are `kokushimuso`. Reading the file backwards from `mjai` gives
  a set, not a yaku. mjai also has no open/closed name variants — the han count
  carries that difference.
- **`nagashimangan` is not an mjai yaku.** mjai ends the hand as `ryukyoku`
  with reason `nagashimangan`; the label is listed so the mapping stays total.
- **Han values are documentation, not the scorer.** They are taken from
  `docs/glossary.md`; the authority on what a hand actually scores is
  `winning_hand::checker`, which computes han per hand rather than from a table.
  Only the names are covered by the drift tests.
- **Unimplemented yaku are absent.** The table is keyed by `Kind`, so Blessing
  of Man (Renhō / 人和), which this project does not implement, has no row.
  `docs/glossary.md` lists it.

### Licence

Same as the rest of the repository. The English names themselves are the WRC's;
the URL above is the citation.
