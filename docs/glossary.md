# Ubiquitous Language Glossary

This document fixes the agreed correspondence between Japanese Riichi Mahjong terms,
their English translations, and the identifiers used in the codebase. It is the
reference for the upcoming i18n work and for any discussion that crosses the
Japanese/English boundary.

A Japanese-language edition of this document is maintained in parallel at
[glossary.ja.md](glossary.ja.md).

## Conventions

- **English** translations follow the **World Riichi Championship (WRC) Rules 2025**
  (reference: <https://www.worldriichi.org/s/WRC-Rules-2025-42fx.pdf>,
  §3 Term Definitions, §11.5 Yaku list).
- **Romaji** is the WRC romanized Japanese reading (with macrons), the most
  portable form within the Riichi community.
- **Code id** is the Rust identifier (enum variant, constant, or method) so a term can
  be traced to its definition. For yaku, [`winning_hand::name`](../crates/mahjong-core/src/winning_hand/name.rs)
  emits the **English** name in this table for `Lang::En`.

---

## Tiles

| Japanese | Romaji | English | Code id | Notes |
|---|---|---|---|---|
| 牌 | hai | tile | [`Tile`](../crates/mahjong-core/src/tile.rs) | One of 34 distinct kinds, 4 copies each (136 total). |
| 数牌 | shūpai | suit tile / numbered tile | `Tile::is_suited` | Tiles numbered 1–9 in the three suits. |
| 萬子 | manzu | characters | `Tile::is_character`, `Tile::M1`–`M9` | ASCII `1m`–`9m`. WRC suit name: *characters*. |
| 筒子 | pinzu | circles | `Tile::is_circle`, `Tile::P1`–`P9` | ASCII `1p`–`9p`. WRC suit name: *circles* (a.k.a. dots). |
| 索子 | sōzu | bamboos | `Tile::is_bamboo`, `Tile::S1`–`S9` | ASCII `1s`–`9s`. WRC suit name: *bamboos*. |
| 字牌 | jihai | honours | `Tile::is_honour`, `Tile::Z1`–`Z7` | Winds + dragons. ASCII `1z`–`7z`. |
| 風牌 | kazehai | wind tiles | `Tile::is_wind`, [`Wind`](../crates/mahjong-core/src/tile.rs) | East/South/West/North. |
| 三元牌 | sangenpai | dragon tiles | `Tile::is_dragon`, [`Dragon`](../crates/mahjong-core/src/tile.rs) | White/Green/Red. |
| 東 | ton | East | `Wind::East`, `Tile::Z1` | |
| 南 | nan | South | `Wind::South`, `Tile::Z2` | |
| 西 | shā | West | `Wind::West`, `Tile::Z3` | |
| 北 | pei | North | `Wind::North`, `Tile::Z4` | |
| 白 | haku | White dragon | `Dragon::White`, `Tile::Z5` | |
| 發 | hatsu | Green dragon | `Dragon::Green`, `Tile::Z6` | The only fully-green honour. |
| 中 | chun | Red dragon | `Dragon::Red`, `Tile::Z7` | |
| 老頭牌 | rōtōhai | terminals / terminal tiles | `Tile::is_1_or_9` | The 1 and 9 of each suit. |
| 中張牌 | chūchanpai | inside tiles | — | Suit tiles numbered 2–8. Basis of *All Inside* (Tan'yao). |
| 么九牌 | yāochūhai | terminals or honours | `Tile::is_1_9_honour` | Terminals + honours. |
| 場風（牌） | bakaze | round wind | `Status::round_wind` | East in the East round, South in the South round. |
| 自風（牌） | jikaze | seat wind | `Status::seat_wind` | The wind assigned to a player for the current hand. |
| 連風（牌） | renfū | double wind | — | A wind that is both the round wind and the seat wind. |
| 役牌 | yakuhai | value honour | — | Round wind, seat wind, or any dragon. See yaku table. |
| ドラ | dora | dora | `tile::dora_indicator_to_dora` | Bonus tile worth +1 han; not a yaku. |
| ドラ表示牌 | dora hyōjihai | dora indicator | `tile::dora_indicator_to_dora` | The revealed tile that points to the actual dora. |
| 裏ドラ | ura dora | ura dora | — | Hidden dora revealed only on a riichi win. |
| 槓ドラ | kan dora | kan dora | — | Extra dora indicator revealed when a quad is made. |
| 赤ドラ / 赤五 | aka dora / aka five | red five | `Tile::new_red`, `Tile::is_red_dora` | A red `5` worth +1 han. |

---

## Groups, melds, and shapes

| Japanese | Romaji | English | Code id | Notes |
|---|---|---|---|---|
| 面子 | mentsu | group | [`Block`](../crates/mahjong-core/src/hand_info/block.rs) | A sequence, triplet, or quad (3–4 tiles). |
| 順子 | shuntsu | sequence | `Sequential3` | Three consecutive tiles, same suit (a.k.a. chow / chii). |
| 刻子 | kōtsu | triplet | `Same3` | Three identical tiles (a.k.a. pung / pon). |
| 槓子 | kantsu | quad | — | Four identical tiles (a.k.a. kong / kan). |
| 対子 | toitsu | pair | `Same2` | Two identical tiles; not a group. |
| 雀頭 | jantō | pair (the head) | — | The pair in a standard 4-groups-and-a-pair hand. |
| 搭子 | tātsu | partial sequence | `Sequential2` | Two tiles one apart, awaiting a third. Not a WRC term. |
| 暗（〜） | an- | concealed | `MeldFrom::Myself` | Formed from self-drawn tiles only. |
| 明（〜） / 副露 | min- / fūro | melded / open | [`Meld`](../crates/mahjong-core/src/hand_info/meld.rs) | Formed by calling a discarded tile. |
| チー | chī | chii (melded sequence) | `MeldType::Chi` | Call a sequence from the player on your left. |
| ポン | pon | pon (melded triplet) | `MeldType::Pon` | Call a triplet from any player. |
| カン | kan | quad call | `MeldType::Kan` | Make a quad. |
| 暗槓 | ankan | concealed quad | `MeldType::Kan` + `MeldFrom::Myself` | Quad from four self-drawn tiles. |
| 大明槓 | daiminkan | called quad | `MeldType::Kan` | Quad completed by calling a discard. |
| 加槓 | kakan | promoted quad | `MeldType::Kakan` | Add a self-drawn tile to a melded triplet. |
| 嶺上牌 | rinshanpai | replacement tile | — | Drawn from the dead wall after declaring a quad. |
| 両面（待ち） | ryanmen | two-sided wait | `Sequential3::is_two_sided_wait` | Open wait on either end of a sequence. |
| 嵌張（待ち） | kanchan | closed wait | — | Waiting on the middle tile of a sequence. |
| 辺張（待ち） | penchan | edge wait | — | Waiting on `3` (from 1-2) or `7` (from 8-9). |
| 単騎（待ち） | tanki | pair wait | — | Waiting to complete the pair. |

---

## Hands and states

| Japanese | Romaji | English | Code id | Notes |
|---|---|---|---|---|
| 手牌 | tehai | hand / player's hand | [`Hand`](../crates/mahjong-core/src/hand.rs) | The player's 13/14 tiles. |
| 門前 | menzen | closed | `Status::has_claimed_open` (false) | No melded groups (ron's last group still counts as closed). |
| 副露 / 鳴き | fūro / naki | open | `Status::has_claimed_open` (true) | Has at least one melded group. |
| 和了 | hōra / agari | winning a hand | — | Completing a valid hand with a yaku. |
| 和了形 | hōrakei | winning hand | — | A valid hand having at least one yaku. |
| 聴牌 | tenpai | tenpai | — | One tile away from a valid hand. |
| 形式聴牌 | keishiki tenpai | keishiki tenpai | — | Yakuless tenpai (allowed at exhaustive draw). |
| 不聴 / ノーテン | noten | noten | — | Not tenpai. |
| 振聴 | furiten | furiten | — | Cannot win by ron (missed a winning tile / discarded a wait). Tracked server-side. |
| 向聴（数） | shanten | shanten | [`HandAnalyzer`](../crates/mahjong-core/src/hand_info/hand_analyzer.rs) | Number of tiles away from tenpai. |
| 七対子（形） | chiitoitsu | seven pairs (form) | `Form::SevenPairs` | A hand of seven distinct pairs. |
| 国士無双（形） | kokushi musō | thirteen orphans (form) | `Form::ThirteenOrphans` | The 13-terminals-and-honours form. |

---

## Gameplay

| Japanese | Romaji | English | Code id | Notes |
|---|---|---|---|---|
| 局 | kyoku | hand (gameplay division) | — | One deal, from East's first discard to the win/draw. |
| 場 | ba | round | — | A division of four+ hands named after a wind (East, South). |
| 半荘 | hanchan | hanchan / game | — | The East and South rounds together. |
| 巡（目） | jun | turn | — | From drawing/calling to discarding. |
| 下家 | shimocha | right player | — | The player to your right; draws immediately after you. |
| 対面 | toimen | across player | — | The player seated opposite you. |
| 上家 | kamicha | left player | — | The player to your left; draws immediately before you. The only player you can call chii from. |
| 牌山 | haiyama | wall | — | The 136 tiles arranged as the drawing pile. |
| 王牌 | wanpai | dead wall | — | The last 14 tiles; dora indicators + replacement tiles. |
| 河 / 捨て牌 | ho / sutehai | discard pool | — | The tiles a player has discarded. |
| 自摸 | tsumo | self-draw | `Status::is_self_drawn` | Drawing a tile from the wall. |
| 立直 | riichi | riichi | `Status::has_claimed_riichi` | Closed-hand ready declaration; 1,000-point deposit. |
| ロン | ron | win by calling a tile / ron | — | Completing the hand on a discard. |
| 放銃 | hōjū | deal-in | — | Discarding the tile another player wins on by ron. |
| ツモ（和了） | tsumo | win by self-draw / tsumo | — | Completing the hand on a self-draw. |
| 流局 | ryūkyoku | exhaustive draw | — | The live wall is exhausted with no winner. |
| 本場 | honba | continuance counter | — | Adds 300 points to the next win. Tracked server-side. |
| 供託 / リーチ棒 | kyōtaku / riichi-bō | riichi deposit | — | The 1,000-point fee paid on riichi; taken by the next winner. |
| 包 / 責任払い | pao / sekinin-barai | liability payment | — | A feeder pays the full value of Big Dragons / Big Winds / Four Quads. |
| 喰い替え | kuikae | swap-calling | — | Calling then discarding the same/equivalent tile. Not yet implemented. |
| 喰いタン | kuitan | Open Tan'yao | `Settings::opened_all_inside` | Whether All Inside (Tan'yao) is allowed on an open hand. |
| 四槓散了 | sūkan sanra | four-quads abortive draw | `Settings::four_kans_draw` | Optional rule. |
| 四風連打 | sūfon renda | four-winds abortive draw | `Settings::four_winds_draw` | Optional rule. |
| 四家立直 | sūcha riichi | four-riichi abortive draw | `Settings::four_riichi_draw` | Optional rule. |
| 九種九牌 | kyūshu kyūhai | nine terminals abortive draw | `Settings::nine_terminals_draw` | Optional rule. |
| 三家和 | sanchahō | triple-ron abortive draw | `Settings::triple_ron_draw` | Optional rule. |

---

## Scoring

| Japanese | Romaji | English | Code id | Notes |
|---|---|---|---|---|
| 役 | yaku | yaku | [`Kind`](../crates/mahjong-core/src/winning_hand/name.rs) | A scoring pattern; gives one or more han. |
| 翻 | han | han | `ScoreResult::han` | One of the two scoring units, from yaku and dora. |
| 符 | fu | minipoints / fu | `ScoreResult::fu`, [`fu`](../crates/mahjong-core/src/scoring/fu.rs) | The other scoring unit, from groups/pairs/wins. |
| 親 | oya | dealer / East player | — | `dealer_*` fields in `ScoreResult`. |
| 子 | ko | non-dealer | — | `non_dealer_*` fields in `ScoreResult`. |
| 満貫 | mangan | mangan | `ScoreRank::Mangan` | |
| 跳満 | haneman | haneman | `ScoreRank::Haneman` | |
| 倍満 | baiman | baiman | `ScoreRank::Baiman` | |
| 三倍満 | sanbaiman | sanbaiman | `ScoreRank::Sanbaiman` | |
| 四倍満 | yonbaiman | yonbaiman | — | |
| 役満 | yakuman | yakuman | `ScoreRank::Yakuman` | |
| 数え役満 | kazoe yakuman | counted yakuman | — | 13+ han from ordinary yaku/dora. |
| 切り上げ満貫 | kiriage mangan | mangan rounding up | `determine_rank` | 4 han 30 fu / 3 han 60 fu round up to mangan. |
| ウマ | uma | uma | — | End-of-hanchan placement bonus/penalty. |
| オカ | oka | oka | — | Top-place bonus. |

---

## Yaku

English and romaji follow WRC Rules 2025 §11.5 / §13.3. Han values are written
`closed / open`; a single number means it does not change when open. A blank **Code id**
means the yaku is not currently implemented in the codebase.

### 1 han

| Japanese | Romaji | English | Code id (`Kind::…`) | Han | Notes |
|---|---|---|---|---|---|
| 立直 | Riichi | Riichi | `Riichi` | 1 | Closed only. |
| 一発 | Ippatsu | Unbroken | `Unbroken` | 1 | Closed only. Win within one go-around of riichi. |
| 門前清自摸和 | Menzen Tsumo | Fully Concealed Hand | `FullyConcealedHand` | 1 | Closed only; self-draw. |
| 平和 | Pinfu | Pinfu | `Pinfu` | 1 | Closed only. |
| 一盃口 | Iipeikō | Twin Sequences | `TwinSequences` | 1 | Closed only. |
| 断么九 | Tan'yao | All Inside | `AllInside` | 1 | Open hand allowed/disallowed via `Settings::opened_all_inside` (default: allowed). |
| 役牌（場風牌） | Yakuhai (bakaze) | Value Honour (round wind) | `ValueHonourRoundWind` | 1 | |
| 役牌（自風牌） | Yakuhai (jikaze) | Value Honour (seat wind) | `ValueHonourSeatWind` | 1 | |
| 役牌（白） | Yakuhai (haku) | Value Honour (White dragon) | `ValueHonourWhiteDragon` | 1 | |
| 役牌（發） | Yakuhai (hatsu) | Value Honour (Green dragon) | `ValueHonourGreenDragon` | 1 | |
| 役牌（中） | Yakuhai (chun) | Value Honour (Red dragon) | `ValueHonourRedDragon` | 1 | |
| 搶槓 | Chankan | Robbing a Quad | `RobbingAQuad` | 1 | Counts as a ron win. |
| 嶺上開花 | Rinshan Kaihō | After a Quad | `AfterAQuad` | 1 | Counts as a tsumo win. |
| 海底撈月 | Haitei | Last Tile Draw | `LastTileDraw` | 1 | Self-draw on the last live-wall tile. |
| 河底撈魚 | Hōtei | Last Tile Claim | `LastTileClaim` | 1 | Ron on the final discard. |

### 2 han

| Japanese | Romaji | English | Code id (`Kind::…`) | Han | Notes |
|---|---|---|---|---|---|
| ダブル立直 | Daburu Riichi | Double Riichi | `DoubleRiichi` | 2 | Closed only. Riichi on the first discard. |
| 七対子 | Chiitoitsu | Seven Pairs | `SevenPairs` | 2 | Closed only; always 25 fu. |
| 一気通貫 | Ikkitsūkan / Ittsū | Full Straight | `FullStraight` | 2 / 1 | |
| 三色同順 | Sanshoku Dōjun | Mixed Sequences | `MixedSequences` | 2 / 1 | |
| 三色同刻 | Sanshoku Dōkō | Mixed Triplets | `MixedTriplets` | 2 | |
| 対々和 | Toitoi | All Triplets | `AllTriplets` | 2 | |
| 三暗刻 | San'ankō | Three Concealed Triplets | `ThreeConcealedTriplets` | 2 | |
| 三槓子 | Sankantsu | Three Quads | — | 2 | **Not implemented.** |
| 混全帯么九 | Chanta | Common Ends | `CommonEnds` | 2 / 1 | |
| 混老頭 | Honrōtō | Common Terminals | `CommonTerminals` | 2 | |
| 小三元 | Shōsangen | Little Dragons | `LittleDragons` | 2 | Plus 2 han from the dragon Value Honours. |

### 3 han

| Japanese | Romaji | English | Code id (`Kind::…`) | Han | Notes |
|---|---|---|---|---|---|
| 二盃口 | Ryanpeikō | Double Twin Sequences | `DoubleTwinSequences` | 3 | Closed only. |
| 混一色 | Hon'itsu | Common Flush | `CommonFlush` | 3 / 2 | |
| 純全帯么九 | Junchan | Perfect Ends | `PerfectEnds` | 3 / 2 | |

### 5 han

| Japanese | Romaji | English | Code id (`Kind::…`) | Han | Notes |
|---|---|---|---|---|---|
| 人和 | Renhō | Blessing of Man | — | mangan | Closed only. Not implemented. |

### 6 han

| Japanese | Romaji | English | Code id (`Kind::…`) | Han | Notes |
|---|---|---|---|---|---|
| 清一色 | Chin'itsu | Perfect Flush | `PerfectFlush` | 6 / 5 | |

### Yakuman

| Japanese | Romaji | English | Code id (`Kind::…`) | Notes |
|---|---|---|---|---|
| 天和 | Tenhō | Blessing of Heaven | `BlessingOfHeaven` | Closed only; dealer self-draw on the deal. |
| 地和 | Chihō | Blessing of Earth | `BlessingOfEarth` | Closed only; non-dealer first-draw win. |
| 国士無双 | Kokushi Musō | Thirteen Orphans | `ThirteenOrphans` | Closed only. |
| 九蓮宝燈 | Chūren Pōto | Nine Gates | `NineGates` | Closed only. |
| 緑一色 | Ryūiisō | All Green | `AllGreen` | Green dragon not required. |
| 四暗刻 | Sūankō | Four Concealed Triplets | `FourConcealedTriplets` | Closed only. |
| 四暗刻単騎 | Sūankō tanki | Four Concealed Triplets (pair wait) | `FourConcealedTripletsPairWait` | Single-wait variant of Four Concealed Triplets. |
| 四槓子 | Sūkantsu | Four Quads | `FourQuads` | Liability payment may apply. |
| 清老頭 | Chinrōtō | Perfect Terminals | `PerfectTerminals` | |
| 字一色 | Tsūiisō | All Honours | `AllHonours` | |
| 大三元 | Daisangen | Big Dragons | `BigDragons` | Liability payment may apply. |
| 小四喜 | Shōsūshii | Little Winds | `LittleWinds` | |
| 大四喜 | Daisūshii | Big Winds | `BigWinds` | Liability payment may apply. |

### Other

| Japanese | Romaji | English | Code id (`Kind::…`) | Notes |
|---|---|---|---|---|
| 流し満貫 | Nagashi Mangan | Nagashi Mangan | `NagashiMangan` | Worth mangan. |
