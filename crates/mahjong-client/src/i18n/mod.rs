//! クライアントUIの多言語化（i18n）
//!
//! 表示言語は [`mahjong_core::settings::Lang`] を再利用する。固定の文言は
//! [`Key`] 列挙型にキーごとへ全言語まとめて定義し（翻訳の取りこぼし防止）、
//! 数値や牌などを含む可変の文言は [`Translator`] のメソッドで組み立てる
//! （言語ごとの語順差を吸収するため）。
//!
//! 役名・点数等級・風・ドラなどゲーム由来の用語は `mahjong-core` 側の
//! ローカライズ関数を正典とし、ここでは UI 固有の文言のみを扱う。

use mahjong_core::settings::Lang;
use mahjong_core::tile::Wind;
use mahjong_server::cpu::client::{CpuLevel, CpuPersonality};
use mahjong_server::protocol::DrawReason;

/// 現在の表示言語を保持し、文言を解決する軽量ハンドル。
///
/// [`GameState`](crate::game::GameState) が保持し、描画関数へ `&GameState`
/// 経由で渡る。`Copy` なので自由に複製してよい。
#[derive(Debug, Clone, Copy)]
pub struct Translator {
    lang: Lang,
}

impl Translator {
    /// 指定言語の [`Translator`] を作る。
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// 固定文言を解決する。
    pub fn get(&self, key: Key) -> &'static str {
        key.text(self.lang)
    }

    /// CPU の強さラベル（0=弱い, 1=普通, 2=強い）。
    pub fn strength_label(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["弱い", "普通", "強い"],
            Lang::En => ["Weak", "Normal", "Strong"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// CPU の性格ラベル（0=バランス, 1=スピード, 2=高得点, 3=守備的）。
    pub fn personality_label(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["バランス", "スピード", "高得点", "守備的"],
            Lang::En => ["Balanced", "Speedy", "High Value", "Defensive"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// 自分から見た相対座席名（0=下家, 1=対面, 2=上家）。
    pub fn seat_relative(&self, idx: usize) -> &'static str {
        match self.lang {
            Lang::Ja => ["下家", "対面", "上家"],
            Lang::En => ["Right", "Across", "Left"],
        }
        .get(idx)
        .copied()
        .unwrap_or("")
    }

    /// 局表示（例: 日「東1局」/ 英「East 1」）。`round_number` は 0 始まり。
    pub fn round_label(&self, round_number: usize) -> String {
        let wind = Wind::from_index(round_number / 4).name(self.lang);
        let num = (round_number % 4) + 1;
        match self.lang {
            Lang::Ja => format!("{wind}{num}局"),
            Lang::En => format!("{wind} {num}"),
        }
    }

    /// 残り枚数（例: 日「{n}枚」/ 英「{n} tiles」）。上部バー用。
    pub fn wall_count(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("{n}枚"),
            Lang::En => format!("{n} tiles"),
        }
    }

    /// 残り枚数を強調する表記（例: 日「残{n}枚」/ 英「{n} left」）。中央表示用。
    pub fn wall_remaining(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("残{n}枚"),
            Lang::En => format!("{n} left"),
        }
    }

    /// 翻数（例: 日「{n}飜」/ 英「{n} han」）。
    pub fn han(&self, n: u32) -> String {
        match self.lang {
            Lang::Ja => format!("{n}飜"),
            Lang::En => format!("{n} han"),
        }
    }

    /// 翻符のまとめ表記（例: 日「{han}飜 {fu}符」/ 英「{han} han {fu} fu」）。
    pub fn han_fu(&self, han: u32, fu: u32) -> String {
        match self.lang {
            Lang::Ja => format!("{han}飜 {fu}符"),
            Lang::En => format!("{han} han {fu} fu"),
        }
    }

    /// 供託リーチ棒の本数（例: 日「供託 {n}本」/ 英「Deposit {n}」）。
    pub fn riichi_deposit(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("供託 {n}本"),
            Lang::En => format!("Deposit {n}"),
        }
    }

    /// 点数表記（例: 日「{s}点」/ 英「{s} pts」）。`s` は桁区切り済みの文字列。
    pub fn points(&self, s: &str) -> String {
        match self.lang {
            Lang::Ja => format!("{s}点"),
            Lang::En => format!("{s} pts"),
        }
    }

    /// ロビーのルームコード見出し（例: 日「ルームコード  {code}」/ 英「Room code  {code}」）。
    pub fn room_code(&self, code: &str) -> String {
        match self.lang {
            Lang::Ja => format!("ルームコード  {code}"),
            Lang::En => format!("Room code  {code}"),
        }
    }

    /// CPU の強さ名（`CpuLevel` から）。
    pub fn cpu_level_name(&self, level: CpuLevel) -> &'static str {
        let idx = match level {
            CpuLevel::Weak => 0,
            CpuLevel::Normal => 1,
            CpuLevel::Strong => 2,
        };
        self.strength_label(idx)
    }

    /// CPU の性格名（`CpuPersonality` から）。
    pub fn cpu_personality_name(&self, personality: CpuPersonality) -> &'static str {
        let idx = match personality {
            CpuPersonality::Balanced => 0,
            CpuPersonality::Speedy => 1,
            CpuPersonality::HighValue => 2,
            CpuPersonality::Defensive => 3,
        };
        self.personality_label(idx)
    }

    /// ロビーの座席に表示する CPU ラベル（例: 日「CPU (普通・バランス)」/ 英「CPU (Normal, Balanced)」）。
    pub fn cpu_seat_label(&self, level: CpuLevel, personality: CpuPersonality) -> String {
        let lv = self.cpu_level_name(level);
        let ps = self.cpu_personality_name(personality);
        match self.lang {
            Lang::Ja => format!("CPU ({lv}・{ps})"),
            Lang::En => format!("CPU ({lv}, {ps})"),
        }
    }

    /// ロビーの座席行（例: 日「東: {who}{marks}」/ 英「East: {who}{marks}」）。
    pub fn seat_row(&self, wind: Wind, who: &str, marks: &str) -> String {
        format!("{}: {who}{marks}", wind.name(self.lang))
    }

    /// 切断中プレイヤーの席表記（例: 日「{name}（切断中）」/ 英「{name} (offline)」）。
    pub fn disconnected_name(&self, name: &str) -> String {
        format!("{name}{}", self.get(Key::MarkerDisconnected))
    }

    /// 流局理由の名称。
    pub fn draw_reason(&self, reason: DrawReason) -> &'static str {
        match self.lang {
            Lang::Ja => match reason {
                DrawReason::Exhaustive => "荒牌流局",
                DrawReason::FourWinds => "四風連打",
                DrawReason::FourRiichi => "四家立直",
                DrawReason::NineTerminals => "九種九牌",
                DrawReason::FourKans => "四槓散了",
                DrawReason::TripleRon => "三家和",
            },
            Lang::En => match reason {
                DrawReason::Exhaustive => "Exhaustive draw",
                DrawReason::FourWinds => "Four winds",
                DrawReason::FourRiichi => "Four riichi",
                DrawReason::NineTerminals => "Nine terminals",
                DrawReason::FourKans => "Four quads",
                DrawReason::TripleRon => "Triple ron",
            },
        }
    }

    /// 流局の見出し（例: 日「流局（{理由}）」/ 英「Draw ({reason})」）。
    pub fn draw_headline(&self, reason: DrawReason) -> String {
        let reason = self.draw_reason(reason);
        match self.lang {
            Lang::Ja => format!("流局（{reason}）"),
            Lang::En => format!("Draw ({reason})"),
        }
    }

    /// テンパイ者の一覧行（例: 日「テンパイ: {names}」/ 英「Tenpai: {names}」）。
    pub fn tenpai_list(&self, names: &str) -> String {
        match self.lang {
            Lang::Ja => format!("テンパイ: {names}"),
            Lang::En => format!("Tenpai: {names}"),
        }
    }

    /// 供託本数の行（例: 日「供託: {n}本」/ 英「Deposits: {n}」）。
    pub fn deposit_line(&self, n: usize) -> String {
        match self.lang {
            Lang::Ja => format!("供託: {n}本"),
            Lang::En => format!("Deposits: {n}"),
        }
    }

    /// 放銃者の注記（例: 日「（{name}が放銃）」/ 英「 (dealt in by {name})」）。
    pub fn dealt_in_by(&self, name: &str) -> String {
        match self.lang {
            Lang::Ja => format!("（{name}が放銃）"),
            Lang::En => format!(" (dealt in by {name})"),
        }
    }

    /// 和了の見出し（例: 日「{winner}が{type}和了！」/ 英「{winner} wins by {type}!」）。
    pub fn win_headline(&self, winner: &str, win_type: &str) -> String {
        match self.lang {
            Lang::Ja => format!("{winner}が{win_type}和了！"),
            Lang::En => format!("{winner} wins by {win_type}!"),
        }
    }

    /// 手番制限時間の残り秒数（例: 日「残り {n} 秒」/ 英「{n}s left」）。
    pub fn seconds_left(&self, n: u32) -> String {
        match self.lang {
            Lang::Ja => format!("残り {n} 秒"),
            Lang::En => format!("{n}s left"),
        }
    }

    /// 自分の手牌からの暗カン／加カンボタン（例: 日「{tile}カン」/ 英「{tile} Kan」）。
    pub fn kan_with_tile(&self, tile: &str) -> String {
        match self.lang {
            Lang::Ja => format!("{tile}カン"),
            Lang::En => format!("{tile} Kan"),
        }
    }

    /// 順位の接尾辞（日は常に「位」、英は序数接尾辞）。`rank` は 0 始まり。
    pub fn place_suffix(&self, rank: usize) -> &'static str {
        match self.lang {
            Lang::Ja => "位",
            Lang::En => match rank {
                0 => "st",
                1 => "nd",
                2 => "rd",
                _ => "th",
            },
        }
    }
}

/// 引数を取らない固定 UI 文言のキー。
///
/// 各バリアントの訳は [`Key::text`] にまとめて定義する。言語を増やすときは
/// `Lang` に追加し、各 `match` 腕へ訳を足す（コンパイル時に網羅性が保証される）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// 対局設定画面のタイトル
    SetupTitle,
    /// CPU の強さ見出し
    CpuStrengthLabel,
    /// CPU の性格見出し
    CpuPersonalityLabel,
    /// ローカル対局を開始するボタン
    StartGame,
    /// オンライン対戦へ進むボタン
    OnlinePlay,
    /// 対局開始待ちの表示
    GameStarting,
    /// 得点チップなどでの自分の表示名
    You,
    /// 振聴バッジ
    Furiten,
    /// リーチ中バッジ
    RiichiActive,
    /// リーチ宣言牌の選択を促すバッジ
    SelectDiscard,
    /// 選択牌で振聴になる警告バッジ
    WillBeFuriten,
    /// 禁止牌を選択したときの「喰い替えです！」警告バッジ
    IsSwapCalling,
    /// ツモ（自摸和了／ツモ牌ラベル）
    Tsumo,
    /// ロン（出和了）
    Ron,
    /// 次の和了へ進む（複数和了時）
    NextWin,
    /// 次へ進む
    Next,
    /// 流局
    RoundDraw,
    /// ゲーム終了
    GameOver,
    /// もう一度遊ぶ
    PlayAgain,
    /// 和了ボタン
    Win,
    /// 他家の手番待ちヒント
    WaitingOtherPlayer,
    /// リーチ宣言時の打牌選択ヒント
    RiichiSelectHint,
    /// リーチ中の自動ツモ切りヒント
    RiichiAutoDiscard,
    /// 通常時の打牌操作ヒント
    NormalPlayHint,
    /// リーチ宣言ボタン
    Riichi,
    /// 鳴き確認の見出し
    CallPrompt,
    /// ポン
    Pon,
    /// カン
    Kan,
    /// チー
    Chi,
    /// パス
    Pass,
    /// キャンセル
    Cancel,
    /// チーの組み合わせ選択の見出し
    ChiSelectTitle,
    /// ポンの組み合わせ選択の見出し
    PonSelectTitle,
    /// 九種九牌の見出し
    NineTerminals,
    /// 九種九牌で流局するか確認
    DeclareDrawPrompt,
    /// 九種九牌で流局を宣言するボタン
    DeclareDraw,
    /// 続ける（九種九牌で続行）
    Continue,
    /// 名前入力欄の見出し
    NameLabel,
    /// ルームコード入力欄の見出し
    RoomCodeJoinLabel,
    /// ルーム作成ボタン
    CreateRoom,
    /// ルーム参加ボタン
    JoinRoom,
    /// 戻るボタン
    Back,
    /// ロビーの見出し
    Lobby,
    /// ルーム情報取得中の表示
    LoadingRoom,
    /// ルームコード共有の案内
    ShareCodeHint,
    /// 空席はCPUが埋める旨の注記
    EmptySeatsCpu,
    /// ホストの開始待ち表示
    WaitingHost,
    /// 退出ボタン
    Leave,
    /// 座席ラベルの「自分」マーカー
    MarkerYou,
    /// 座席ラベルの「ホスト」マーカー
    MarkerHost,
    /// 座席ラベルの「切断中」マーカー
    MarkerDisconnected,
    /// サーバ接続中の表示
    Connecting,
    /// サーバ切断の表示
    Disconnected,
    /// 空席
    EmptySeat,
    /// 既定の表示名
    DefaultPlayerName,
    /// ルームコードの桁数エラー
    RoomCodeLengthError,
    /// 再接続中の表示
    Reconnecting,
    /// 他プレイヤー切断・CPU代打ちの表示
    PeerDisconnected,
}

impl Key {
    /// 指定言語での文言を返す。
    pub fn text(self, lang: Lang) -> &'static str {
        match self {
            Key::SetupTitle => match lang {
                Lang::Ja => "対局設定",
                Lang::En => "Game Setup",
            },
            Key::CpuStrengthLabel => match lang {
                Lang::Ja => "強さ",
                Lang::En => "Strength",
            },
            Key::CpuPersonalityLabel => match lang {
                Lang::Ja => "性格",
                Lang::En => "Style",
            },
            Key::StartGame => match lang {
                Lang::Ja => "対局開始",
                Lang::En => "Start Game",
            },
            Key::OnlinePlay => match lang {
                Lang::Ja => "オンライン対戦",
                Lang::En => "Online Play",
            },
            Key::GameStarting => match lang {
                Lang::Ja => "ゲーム開始中...",
                Lang::En => "Starting game...",
            },
            Key::You => match lang {
                Lang::Ja => "あなた",
                Lang::En => "You",
            },
            Key::Furiten => match lang {
                Lang::Ja => "振聴",
                Lang::En => "Furiten",
            },
            Key::RiichiActive => match lang {
                Lang::Ja => "リーチ中",
                Lang::En => "Riichi",
            },
            Key::SelectDiscard => match lang {
                Lang::Ja => "打牌を選択",
                Lang::En => "Select a discard",
            },
            Key::WillBeFuriten => match lang {
                Lang::Ja => "振聴になります！",
                Lang::En => "Will cause furiten!",
            },
            Key::IsSwapCalling => match lang {
                Lang::Ja => "喰い替えです！",
                Lang::En => "That's swap-calling!",
            },
            Key::Tsumo => match lang {
                Lang::Ja => "ツモ",
                Lang::En => "Tsumo",
            },
            Key::Ron => match lang {
                Lang::Ja => "ロン",
                Lang::En => "Ron",
            },
            Key::NextWin => match lang {
                Lang::Ja => "次の和了へ →",
                Lang::En => "Next win →",
            },
            Key::Next => match lang {
                Lang::Ja => "次へ →",
                Lang::En => "Next →",
            },
            Key::RoundDraw => match lang {
                Lang::Ja => "流局",
                Lang::En => "Draw",
            },
            Key::GameOver => match lang {
                Lang::Ja => "ゲーム終了",
                Lang::En => "Game Over",
            },
            Key::PlayAgain => match lang {
                Lang::Ja => "もう一度",
                Lang::En => "Play Again",
            },
            Key::Win => match lang {
                Lang::Ja => "和了",
                Lang::En => "Win",
            },
            Key::WaitingOtherPlayer => match lang {
                Lang::Ja => "他のプレイヤーの手番です...",
                Lang::En => "Waiting for other players...",
            },
            Key::RiichiSelectHint => match lang {
                Lang::Ja => "【リーチ】聴牌になる牌を選んで打牌",
                Lang::En => "[Riichi] Discard a tile that keeps you tenpai",
            },
            Key::RiichiAutoDiscard => match lang {
                Lang::Ja => "【リーチ中】自動ツモ切り",
                Lang::En => "[Riichi] Auto-discarding draws",
            },
            Key::NormalPlayHint => match lang {
                Lang::Ja => "牌をクリックで選択、もう一度クリックで打牌",
                Lang::En => "Click a tile to select, click again to discard",
            },
            Key::Riichi => match lang {
                Lang::Ja => "リーチ",
                Lang::En => "Riichi",
            },
            Key::CallPrompt => match lang {
                Lang::Ja => "鳴きますか？",
                Lang::En => "Call?",
            },
            Key::Pon => match lang {
                Lang::Ja => "ポン",
                Lang::En => "Pon",
            },
            Key::Kan => match lang {
                Lang::Ja => "カン",
                // 用語集（docs/glossary.md）に合わせ呼称「kan」を用いる
                Lang::En => "Kan",
            },
            Key::Chi => match lang {
                Lang::Ja => "チー",
                // 用語集（docs/glossary.md）に合わせ "chii" を用いる
                Lang::En => "Chii",
            },
            Key::Pass => match lang {
                Lang::Ja => "パス",
                Lang::En => "Pass",
            },
            Key::Cancel => match lang {
                Lang::Ja => "キャンセル",
                Lang::En => "Cancel",
            },
            Key::ChiSelectTitle => match lang {
                Lang::Ja => "チーの組み合わせを選択",
                Lang::En => "Choose a chii combination",
            },
            Key::PonSelectTitle => match lang {
                Lang::Ja => "ポンの組み合わせを選択",
                Lang::En => "Choose a pon combination",
            },
            Key::NineTerminals => match lang {
                Lang::Ja => "九種九牌",
                Lang::En => "Nine Terminals",
            },
            Key::DeclareDrawPrompt => match lang {
                Lang::Ja => "流局しますか？",
                Lang::En => "Declare a draw?",
            },
            Key::DeclareDraw => match lang {
                Lang::Ja => "流局する",
                Lang::En => "Declare draw",
            },
            Key::Continue => match lang {
                Lang::Ja => "続ける",
                Lang::En => "Continue",
            },
            Key::NameLabel => match lang {
                Lang::Ja => "名前",
                Lang::En => "Name",
            },
            Key::RoomCodeJoinLabel => match lang {
                Lang::Ja => "ルームコード（参加する場合）",
                Lang::En => "Room code (to join)",
            },
            Key::CreateRoom => match lang {
                Lang::Ja => "ルームを作成",
                Lang::En => "Create Room",
            },
            Key::JoinRoom => match lang {
                Lang::Ja => "ルームに参加",
                Lang::En => "Join Room",
            },
            Key::Back => match lang {
                Lang::Ja => "戻る",
                Lang::En => "Back",
            },
            Key::Lobby => match lang {
                Lang::Ja => "ロビー",
                Lang::En => "Lobby",
            },
            Key::LoadingRoom => match lang {
                Lang::Ja => "ルーム情報を取得中...",
                Lang::En => "Loading room...",
            },
            Key::ShareCodeHint => match lang {
                Lang::Ja => "このコードを参加プレイヤーに共有してください",
                Lang::En => "Share this code with the players joining",
            },
            Key::EmptySeatsCpu => match lang {
                Lang::Ja => "空席はCPUが入ります",
                Lang::En => "Empty seats are filled by CPUs",
            },
            Key::WaitingHost => match lang {
                Lang::Ja => "ホストの開始を待っています...",
                Lang::En => "Waiting for the host to start...",
            },
            Key::Leave => match lang {
                Lang::Ja => "退出",
                Lang::En => "Leave",
            },
            Key::MarkerYou => match lang {
                Lang::Ja => "（あなた）",
                Lang::En => " (You)",
            },
            Key::MarkerHost => match lang {
                Lang::Ja => "（ホスト）",
                Lang::En => " (Host)",
            },
            Key::MarkerDisconnected => match lang {
                Lang::Ja => "（切断中）",
                Lang::En => " (offline)",
            },
            Key::Connecting => match lang {
                Lang::Ja => "サーバに接続中...",
                Lang::En => "Connecting to server...",
            },
            Key::Disconnected => match lang {
                Lang::Ja => "サーバとの接続が切れました",
                Lang::En => "Disconnected from server",
            },
            Key::EmptySeat => match lang {
                Lang::Ja => "空席",
                Lang::En => "Empty",
            },
            Key::DefaultPlayerName => match lang {
                Lang::Ja => "プレイヤー",
                Lang::En => "Player",
            },
            Key::RoomCodeLengthError => match lang {
                Lang::Ja => "ルームコードを6文字で入力してください",
                Lang::En => "Enter a 6-character room code",
            },
            Key::Reconnecting => match lang {
                Lang::Ja => "再接続中...",
                Lang::En => "Reconnecting...",
            },
            Key::PeerDisconnected => match lang {
                Lang::Ja => "他のプレイヤーが切断中（CPUが代打ち）",
                Lang::En => "A player disconnected (a CPU is filling in)",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_resolves_both_languages() {
        assert_eq!(Key::StartGame.text(Lang::Ja), "対局開始");
        assert_eq!(Key::StartGame.text(Lang::En), "Start Game");
    }

    #[test]
    fn translator_indexed_labels() {
        let ja = Translator::new(Lang::Ja);
        let en = Translator::new(Lang::En);
        assert_eq!(ja.strength_label(0), "弱い");
        assert_eq!(en.strength_label(2), "Strong");
        assert_eq!(ja.personality_label(3), "守備的");
        assert_eq!(en.personality_label(0), "Balanced");
        assert_eq!(ja.seat_relative(1), "対面");
        assert_eq!(en.seat_relative(2), "Left");
    }

    #[test]
    fn out_of_range_index_is_empty() {
        let t = Translator::new(Lang::En);
        assert_eq!(t.strength_label(9), "");
    }
}
