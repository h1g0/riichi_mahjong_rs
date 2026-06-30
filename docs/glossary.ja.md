# ユビキタス言語集

このドキュメントは、日本リーチ麻雀の用語について「日本語名 / 英語の訳語 /
コードベース上の識別子」の対応を確定させたものです。今後の i18n 対応、および
日本語・英語をまたぐ議論の際の基準とします。

英語版は [glossary.md](glossary.md) に並行して管理しています。

## 表記ルール

- **英語** の訳語は **World Riichi Championship (WRC) Rules 2025** に準拠します
  （出典: <https://www.worldriichi.org/s/WRC-Rules-2025-42fx.pdf>、
  §3 Term Definitions、§11.5 Yaku list）。
- **ローマ字（Romaji）** は WRC のローマ字読み（マクロン付き）です。リーチ麻雀
  コミュニティで最も通用する形です。
- **コード識別子** は定義に辿れるよう Rust の識別子（enum バリアント・定数・
  メソッド）を示します。役については
  [`winning_hand::name`](../crates/mahjong-core/src/winning_hand/name.rs) が
  `Lang::En` でこの表の **英語** 名を出力します。

---

## 牌（Tiles）

| 日本語 | ローマ字 | 英語 | コード識別子 | 備考 |
|---|---|---|---|---|
| 牌 | hai | tile | [`Tile`](../crates/mahjong-core/src/tile.rs) | 34 種、各 4 枚（計 136 枚）。 |
| 数牌 | shūpai | suit tile / numbered tile | `Tile::is_suited` | 三スートの 1〜9。 |
| 萬子 | manzu | characters | `Tile::is_character`, `Tile::M1`–`M9` | ASCII `1m`–`9m`。WRC のスート名は *characters*。 |
| 筒子 | pinzu | circles | `Tile::is_circle`, `Tile::P1`–`P9` | ASCII `1p`–`9p`。WRC のスート名は *circles*（dots とも）。 |
| 索子 | sōzu | bamboos | `Tile::is_bamboo`, `Tile::S1`–`S9` | ASCII `1s`–`9s`。WRC のスート名は *bamboos*。 |
| 字牌 | jihai | honours | `Tile::is_honour`, `Tile::Z1`–`Z7` | 風牌＋三元牌。ASCII `1z`–`7z`。 |
| 風牌 | kazehai | wind tiles | `Tile::is_wind`, [`Wind`](../crates/mahjong-core/src/tile.rs) | 東・南・西・北。 |
| 三元牌 | sangenpai | dragon tiles | `Tile::is_dragon`, [`Dragon`](../crates/mahjong-core/src/tile.rs) | 白・發・中。 |
| 東 | ton | East | `Wind::East`, `Tile::Z1` | |
| 南 | nan | South | `Wind::South`, `Tile::Z2` | |
| 西 | shā | West | `Wind::West`, `Tile::Z3` | |
| 北 | pei | North | `Wind::North`, `Tile::Z4` | |
| 白 | haku | White dragon | `Dragon::White`, `Tile::Z5` | |
| 發 | hatsu | Green dragon | `Dragon::Green`, `Tile::Z6` | 全面が緑の唯一の字牌。 |
| 中 | chun | Red dragon | `Dragon::Red`, `Tile::Z7` | |
| 老頭牌 | rōtōhai | terminals / terminal tiles | `Tile::is_1_or_9` | 各スートの 1 と 9。 |
| 中張牌 | chūchanpai | inside tiles | — | 各スートの 2〜8。断么九（All Inside）の基礎。 |
| 么九牌 | yāochūhai | terminals or honours | `Tile::is_1_9_honour` | 老頭牌＋字牌。 |
| 場風（牌） | bakaze | round wind | `Status::round_wind` | 東場は東、南場は南。 |
| 自風（牌） | jikaze | seat wind | `Status::seat_wind` | その局で各プレイヤーに割り当てられる風。 |
| 連風（牌） | renfū | double wind | — | 場風かつ自風となる風。 |
| 役牌 | yakuhai | value honour | — | 場風・自風・三元牌。役の表を参照。 |
| ドラ | dora | dora | `tile::dora_indicator_to_dora` | +1 翻のボーナス牌。役ではない。 |
| ドラ表示牌 | dora hyōjihai | dora indicator | `tile::dora_indicator_to_dora` | 実際のドラを指し示す公開牌。 |
| 裏ドラ | ura dora | ura dora | — | リーチ和了時のみ公開される裏のドラ。 |
| 槓ドラ | kan dora | kan dora | — | カン成立時に追加で公開されるドラ表示牌。 |
| 赤ドラ / 赤五 | aka dora / aka five | red five | `Tile::new_red`, `Tile::is_red_dora` | +1 翻の赤い `5`。 |

---

## 面子・副露・形（Groups, melds, shapes）

| 日本語 | ローマ字 | 英語 | コード識別子 | 備考 |
|---|---|---|---|---|
| 面子 | mentsu | group | [`Block`](../crates/mahjong-core/src/hand_info/block.rs) | 順子・刻子・槓子（3〜4 枚）。 |
| 順子 | shuntsu | sequence | `Sequential3` | 同スートの連続 3 枚（chow / chii）。 |
| 刻子 | kōtsu | triplet | `Same3` | 同一牌 3 枚（pung / pon）。 |
| 槓子 | kantsu | quad | — | 同一牌 4 枚（kong / kan）。 |
| 対子 | toitsu | pair | `Same2` | 同一牌 2 枚。面子ではない。 |
| 雀頭 | jantō | pair (the head) | — | 4 面子 1 雀頭の通常形における対子。 |
| 搭子 | tātsu | partial sequence | `Sequential2` | 3 枚目を待つ 2 枚。WRC の用語ではない。 |
| 暗（〜） | an- | concealed | `MeldFrom::Myself` | 自摸牌のみで構成。 |
| 明（〜） / 副露 | min- / fūro | melded / open | [`Meld`](../crates/mahjong-core/src/hand_info/meld.rs) | 捨て牌を鳴いて構成。 |
| チー | chī | chii (melded sequence) | `MeldType::Chi` | 上家の捨て牌で順子を作る。 |
| ポン | pon | pon (melded triplet) | `MeldType::Pon` | 任意の他家の捨て牌で刻子を作る。 |
| カン | kan | kan | `MeldType::Kan` | 槓子を作る。鳴きの呼称はポン・チーと同様に借用語「kan」を用い、できる面子は「quad」と呼ぶ。 |
| 暗槓 | ankan | concealed quad | `MeldType::Kan` + `MeldFrom::Myself` | 自摸 4 枚による槓。 |
| 大明槓 | daiminkan | called quad | `MeldType::Kan` | 捨て牌を鳴いて完成させる槓。 |
| 加槓 | kakan | promoted quad | `MeldType::Kakan` | ポンに自摸牌を 1 枚加える。 |
| 嶺上牌 | rinshanpai | replacement tile | — | カン宣言後に王牌から引く牌。 |
| 両面（待ち） | ryanmen | two-sided wait | `Sequential3::is_two_sided_wait` | 順子の両端どちらでも和了れる待ち。 |
| 嵌張（待ち） | kanchan | closed wait | — | 順子の中の 1 枚を待つ。 |
| 辺張（待ち） | penchan | edge wait | — | 1-2 から `3`、8-9 から `7` を待つ。 |
| 単騎（待ち） | tanki | pair wait | — | 雀頭の完成を待つ。 |

---

## 手牌・状態（Hands and states）

| 日本語 | ローマ字 | 英語 | コード識別子 | 備考 |
|---|---|---|---|---|
| 手牌 | tehai | hand / player's hand | [`Hand`](../crates/mahjong-core/src/hand.rs) | プレイヤーの 13/14 枚。 |
| 門前 | menzen | closed | `Status::has_claimed_open`（false） | 副露なし（ロンの最終面子は門前扱い）。 |
| 副露 / 鳴き | fūro / naki | open | `Status::has_claimed_open`（true） | 1 つ以上の副露がある。 |
| 和了 | hōra / agari | winning a hand | — | 役のある有効な手を完成させること。 |
| 和了形 | hōrakei | winning hand | — | 役を 1 つ以上持つ有効な手。 |
| 聴牌 | tenpai | tenpai | — | あと 1 枚で有効な手になる状態。 |
| 形式聴牌 | keishiki tenpai | keishiki tenpai | — | 役なしの聴牌（流局時に認められる）。 |
| 不聴 / ノーテン | noten | noten | — | 聴牌していない状態。 |
| 振聴 | furiten | furiten | — | ロン和了できない状態（和了牌見逃し・待ち牌が河にある等）。サーバ側で管理。 |
| 向聴（数） | shanten | shanten | [`HandAnalyzer`](../crates/mahjong-core/src/hand_info/hand_analyzer.rs) | 聴牌までの距離。 |
| 七対子（形） | chiitoitsu | seven pairs (form) | `Form::SevenPairs` | 7 つの異なる対子による形。 |
| 国士無双（形） | kokushi musō | thirteen orphans (form) | `Form::ThirteenOrphans` | 13 種の么九牌による形。 |

---

## ゲーム進行（Gameplay）

| 日本語 | ローマ字 | 英語 | コード識別子 | 備考 |
|---|---|---|---|---|
| 局 | kyoku | hand (gameplay division) | — | 親の第一打から和了/流局までの 1 配。 |
| 場 | ba | round | — | 風名を冠する 4 局以上の区切り（東・南）。 |
| 半荘 | hanchan | hanchan / game | — | 東場と南場を合わせたもの。 |
| 巡（目） | jun | turn | — | ツモ/鳴きから打牌まで。 |
| 下家 | shimocha | right player | — | 自分の右隣（自分の次に打つ）プレイヤー。 |
| 対面 | toimen | across player | — | 自分の正面のプレイヤー。 |
| 上家 | kamicha | left player | — | 自分の左隣（自分の前に打つ）プレイヤー。チーできる唯一の相手。 |
| 牌山 | haiyama | wall | — | 136 枚を山に積んだもの。 |
| 王牌 | wanpai | dead wall | — | 末尾 14 枚。ドラ表示牌＋嶺上牌。 |
| 河 / 捨て牌 | ho / sutehai | discard pool | — | プレイヤーが捨てた牌。 |
| 自摸 | tsumo | self-draw | `Status::is_self_drawn` | 山から牌を引くこと。 |
| 立直 | riichi | riichi | `Status::has_claimed_riichi` | 門前聴牌の宣言。1,000 点の供託。 |
| ロン | ron | win by calling a tile / ron | — | 捨て牌で和了すること。 |
| 放銃 | hōjū | deal-in | — | 他家のロン和了牌を捨てること。 |
| ツモ（和了） | tsumo | win by self-draw / tsumo | — | 自摸で和了すること。 |
| 流局 | ryūkyoku | exhaustive draw | — | 和了者なしで生牌が尽きること。 |
| 本場 | honba | continuance counter | — | 次局の和了に 300 点加算。サーバ側で管理。 |
| 供託 / リーチ棒 | kyōtaku / riichi-bō | riichi deposit | — | リーチ時に払う 1,000 点。次の和了者が取得。 |
| 包 / 責任払い | pao / sekinin-barai | liability payment | — | 大三元・大四喜・四槓子で確定牌を放銃した者が全額払い。 |
| 喰い替え | kuikae | swap-calling | `Settings::forbid_swap_calling` | 鳴いた牌と同/同等の牌を即捨てすること。デフォルトで禁止。 |
| 喰いタン | kuitan | Open Tan'yao | `Settings::opened_all_inside` | 副露した手で断么九（All Inside）を認めるか否か。 |
| 四槓散了 | sūkan sanra | four-quads abortive draw | `Settings::four_kans_draw` | オプション。 |
| 四風連打 | sūfon renda | four-winds abortive draw | `Settings::four_winds_draw` | オプション。 |
| 四家立直 | sūcha riichi | four-riichi abortive draw | `Settings::four_riichi_draw` | オプション。 |
| 九種九牌 | kyūshu kyūhai | nine terminals abortive draw | `Settings::nine_terminals_draw` | オプション。 |
| 三家和 | sanchahō | triple-ron abortive draw | `Settings::triple_ron_draw` | オプション。 |

---

## 点数計算（Scoring）

| 日本語 | ローマ字 | 英語 | コード識別子 | 備考 |
|---|---|---|---|---|
| 役 | yaku | yaku | [`Kind`](../crates/mahjong-core/src/winning_hand/name.rs) | 点数となる形。1 翻以上を与える。 |
| 翻 | han | han | `ScoreResult::han` | 2 つの計算単位の一方。役とドラから。 |
| 符 | fu | minipoints / fu | `ScoreResult::fu`, [`fu`](../crates/mahjong-core/src/scoring/fu.rs) | もう一方の計算単位。面子・雀頭・和了から。 |
| 親 | oya | dealer / East player | — | `ScoreResult` の `dealer_*` フィールド。 |
| 子 | ko | non-dealer | — | `ScoreResult` の `non_dealer_*` フィールド。 |
| 満貫 | mangan | mangan | `ScoreRank::Mangan` | |
| 跳満 | haneman | haneman | `ScoreRank::Haneman` | |
| 倍満 | baiman | baiman | `ScoreRank::Baiman` | |
| 三倍満 | sanbaiman | sanbaiman | `ScoreRank::Sanbaiman` | |
| 四倍満 | yonbaiman | yonbaiman | — | |
| 役満 | yakuman | yakuman | `ScoreRank::Yakuman` | |
| 数え役満 | kazoe yakuman | counted yakuman | — | 通常役・ドラで 13 翻以上。 |
| 切り上げ満貫 | kiriage mangan | mangan rounding up | `determine_rank` | 4 翻 30 符・3 翻 60 符を満貫に切り上げ。 |
| ウマ | uma | uma | — | 半荘終了時の順位ボーナス/ペナルティ。 |
| オカ | oka | oka | — | トップ賞。 |

---

## 役（Yaku）

英語とローマ字は WRC Rules 2025 §11.5 / §13.3 に準拠。翻数は `門前 / 副露` で
表記し、数字が 1 つの場合は鳴いても変化しません。**コード識別子**が空欄の役は
コードベースで現在未実装です。

### 1 翻

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 翻 | 備考 |
|---|---|---|---|---|---|
| 立直 | Riichi | Riichi | `Riichi` | 1 | 門前のみ。 |
| 一発 | Ippatsu | Unbroken | `Unbroken` | 1 | 門前のみ。リーチ後 1 巡以内の和了。 |
| 門前清自摸和 | Menzen Tsumo | Fully Concealed Hand | `FullyConcealedHand` | 1 | 門前のみ・自摸。 |
| 平和 | Pinfu | Pinfu | `Pinfu` | 1 | 門前のみ。 |
| 一盃口 | Iipeikō | Twin Sequences | `TwinSequences` | 1 | 門前のみ。 |
| 断么九 | Tan'yao | All Inside | `AllInside` | 1 | 喰いタンの有無は設定可能（`Settings::opened_all_inside`、デフォルトはあり）。 |
| 役牌（場風牌） | Yakuhai (bakaze) | Value Honour (round wind) | `ValueHonourRoundWind` | 1 | |
| 役牌（自風牌） | Yakuhai (jikaze) | Value Honour (seat wind) | `ValueHonourSeatWind` | 1 | |
| 役牌（白） | Yakuhai (haku) | Value Honour (White dragon) | `ValueHonourWhiteDragon` | 1 | |
| 役牌（發） | Yakuhai (hatsu) | Value Honour (Green dragon) | `ValueHonourGreenDragon` | 1 | |
| 役牌（中） | Yakuhai (chun) | Value Honour (Red dragon) | `ValueHonourRedDragon` | 1 | |
| 搶槓 | Chankan | Robbing a Quad | `RobbingAQuad` | 1 | ロン和了扱い。 |
| 嶺上開花 | Rinshan Kaihō | After a Quad | `AfterAQuad` | 1 | 自摸和了扱い。 |
| 海底撈月 | Haitei | Last Tile Draw | `LastTileDraw` | 1 | 生牌最後の 1 枚での自摸。 |
| 河底撈魚 | Hōtei | Last Tile Claim | `LastTileClaim` | 1 | 最終打牌でのロン。 |

### 2 翻

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 翻 | 備考 |
|---|---|---|---|---|---|
| ダブル立直 | Daburu Riichi | Double Riichi | `DoubleRiichi` | 2 | 門前のみ。第一打でのリーチ。 |
| 七対子 | Chiitoitsu | Seven Pairs | `SevenPairs` | 2 | 門前のみ・常に 25 符。 |
| 一気通貫 | Ikkitsūkan / Ittsū | Full Straight | `FullStraight` | 2 / 1 | |
| 三色同順 | Sanshoku Dōjun | Mixed Sequences | `MixedSequences` | 2 / 1 | |
| 三色同刻 | Sanshoku Dōkō | Mixed Triplets | `MixedTriplets` | 2 | |
| 対々和 | Toitoi | All Triplets | `AllTriplets` | 2 | |
| 三暗刻 | San'ankō | Three Concealed Triplets | `ThreeConcealedTriplets` | 2 | |
| 三槓子 | Sankantsu | Three Quads | — | 2 | **未実装。** |
| 混全帯么九 | Chanta | Common Ends | `CommonEnds` | 2 / 1 | |
| 混老頭 | Honrōtō | Common Terminals | `CommonTerminals` | 2 | |
| 小三元 | Shōsangen | Little Dragons | `LittleDragons` | 2 | 三元牌の役牌で別途 +2 翻。 |

### 3 翻

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 翻 | 備考 |
|---|---|---|---|---|---|
| 二盃口 | Ryanpeikō | Double Twin Sequences | `DoubleTwinSequences` | 3 | 門前のみ。 |
| 混一色 | Hon'itsu | Common Flush | `CommonFlush` | 3 / 2 | |
| 純全帯么九 | Junchan | Perfect Ends | `PerfectEnds` | 3 / 2 | |

### 5 翻

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 翻 | 備考 |
|---|---|---|---|---|---|
| 人和 | Renhō | Blessing of Man | — | 満貫 | 門前のみ。未実装。 |

### 6 翻

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 翻 | 備考 |
|---|---|---|---|---|---|
| 清一色 | Chin'itsu | Perfect Flush | `PerfectFlush` | 6 / 5 | |

### 役満

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 備考 |
|---|---|---|---|---|
| 天和 | Tenhō | Blessing of Heaven | `BlessingOfHeaven` | 門前のみ。親が配牌時の自摸で和了。 |
| 地和 | Chihō | Blessing of Earth | `BlessingOfEarth` | 門前のみ。子が第一自摸で和了。 |
| 国士無双 | Kokushi Musō | Thirteen Orphans | `ThirteenOrphans` | 門前のみ。 |
| 九蓮宝燈 | Chūren Pōto | Nine Gates | `NineGates` | 門前のみ。 |
| 緑一色 | Ryūiisō | All Green | `AllGreen` | 發は必須ではない。 |
| 四暗刻 | Sūankō | Four Concealed Triplets | `FourConcealedTriplets` | 門前のみ。 |
| 四暗刻単騎 | Sūankō tanki | Four Concealed Triplets (pair wait) | `FourConcealedTripletsPairWait` | 四暗刻の単騎待ちバリアント。 |
| 四槓子 | Sūkantsu | Four Quads | `FourQuads` | 責任払いの対象になりうる。 |
| 清老頭 | Chinrōtō | Perfect Terminals | `PerfectTerminals` | |
| 字一色 | Tsūiisō | All Honours | `AllHonours` | |
| 大三元 | Daisangen | Big Dragons | `BigDragons` | 責任払いの対象になりうる。 |
| 小四喜 | Shōsūshii | Little Winds | `LittleWinds` | |
| 大四喜 | Daisūshii | Big Winds | `BigWinds` | 責任払いの対象になりうる。 |

### その他

| 日本語 | ローマ字 | 英語 | コード識別子（`Kind::…`） | 備考 |
|---|---|---|---|---|
| 流し満貫 | Nagashi Mangan | Nagashi Mangan | `NagashiMangan` | 満貫扱い。 |
