//! English and Japanese key help for the full-screen client.
pub(super) const HELP: &[(&str, &str, &str)] = &[
    (
        "1…4 / tab",
        "Home / Notifications / Profile / Chat",
        "ホーム / 通知 / プロフィール / チャット",
    ),
    ("j k / ↓ ↑", "move the selection", "選択を移動"),
    ("g G / home end", "first / last note", "最初 / 最後のノート"),
    (
        "pgdn pgup",
        "move five notes; more loads near the end",
        "5件移動・末尾近くで続きを取得",
    ),
    ("R F5", "refresh", "再読み込み"),
    (
        "v / V / esc",
        "open thread / quoted note / back",
        "スレッド / 引用先 / 戻る",
    ),
    ("n / r", "new note / reply", "新規ノート / 返信"),
    (
        "ctrl+s",
        "send note / message; save profile",
        "投稿 / メッセージ送信・プロフィール保存",
    ),
    (
        "ctrl+u / esc",
        "clear line / cancel composer",
        "行頭まで削除 / 投稿を中止",
    ),
    (
        "l",
        "reaction / deletion request",
        "リアクション / 取消要求",
    ),
    ("b", "repost", "リポスト"),
    (
        "c",
        "copy nostr:nevent identifier",
        "nostr:nevent 識別子をコピー",
    ),
    ("space", "open pictures / video", "画像 / 動画を開く"),
    (
        "h l / ← →",
        "previous / next picture in viewer",
        "画像ビューアで前 / 次の画像",
    ),
    ("s / T", "settings / themes", "設定 / テーマ"),
    (
        "j k / enter",
        "choose a setting or theme",
        "設定やテーマを選択・決定",
    ),
    ("x", "reset the selected setting", "選択中の設定を初期化"),
    ("? / esc", "help / close", "ヘルプ / 閉じる"),
    (
        "enter / esc",
        "author profile / back",
        "作者プロフィール / 戻る",
    ),
    (
        "/ / m / i",
        "search / chat recipient / message",
        "検索 / チャット宛先 / 入力",
    ),
    (
        "Esc",
        "back to timeline from search results",
        "検索結果からタイムラインに戻る",
    ),
    (". / A", "actions / accounts", "操作一覧 / アカウント"),
    (
        "e",
        "edit own profile / reaction",
        "自分のプロフィール編集 / リアクション",
    ),
    (
        "Q / w",
        "quote / reveal content warning",
        "引用 / 閲覧注意を開く",
    ),
    (
        "ctrl+o / tab / ctrl+x",
        "attach / alt text (shift+tab previous) / remove attachment",
        "添付 / 代替テキスト（shift+tab 前） / 添付を除去",
    ),
    ("ctrl+w", "edit content warning", "閲覧注意文を編集"),
    (
        "D / f / M B / !",
        "delete / follow / mute / public report (confirm)",
        "削除 / フォロー / ミュート / 公開通報（確認あり）",
    ),
    (
        "d / viewer r",
        "save attachments / replay video",
        "添付を保存 / 動画を再生",
    ),
    (
        "+ / H L / < >",
        "pin search / select column / reorder",
        "検索を固定 / 列を選択 / 並べ替え",
    ),
    (
        "I",
        "publish configured DM relay list (confirm)",
        "設定済みDMリレー一覧を公開（確認あり）",
    ),
    ("q ctrl+c", "quit", "終了"),
];

use super::app::{App, Overlay};
pub(super) fn hint(app: &App) -> &str {
    if matches!(
        app.extra.dialog,
        Some(super::extras::Dialog::Input {
            purpose: super::extras::Purpose::Search,
            ..
        })
    ) {
        return app.t("Enter search · Esc cancel", "Enter 検索 · Esc キャンセル");
    }
    if app.searching()
        && app.threads.is_empty()
        && app.overlay.is_none()
        && app.extra.dialog.is_none()
    {
        return app.t("Esc back to timeline", "Esc タイムラインに戻る");
    }
    match app.overlay {
        Some(Overlay::Compose { .. }) => app.t("ctrl+s send   esc cancel", "ctrl+s 送信   esc 中止"),
        Some(Overlay::Themes { .. }) => app.t("j k preview   enter apply   esc cancel", "j k プレビュー   enter 適用   esc 中止"),
        Some(Overlay::Settings(_)) => app.t("j k move   enter choose   x reset   esc close", "j k 移動   enter 選択   x 初期化   esc 閉じる"),
        Some(Overlay::Languages(_)) => app.t("j k move   enter choose   esc close", "j k 移動   enter 選択   esc 閉じる"),
        Some(Overlay::Pictures { .. }) => app.t("h l previous / next   d save   r replay   esc close", "h l 前 / 次   d 保存   r 再生   esc 閉じる"),
        Some(Overlay::Help(_)) => app.t("j k scroll   esc close", "j k スクロール   esc 閉じる"),
        None => app.t("? help   / search   1–4 tabs   j k move   tab next tab   n post   r reply   l like   b repost   v thread   c copy   s settings   T themes   q quit", "? ヘルプ   / 検索   1–4 タブ   j k 移動   tab タブ   n 投稿   r 返信   l リアクション   b リポスト   v スレッド   c コピー   s 設定   T テーマ   q 終了"),
    }
}

pub(super) fn browser_ja(value: &str) -> &str {
    match value {
        "Attach pictures (up to {}) or a video" => "画像（最大 {} 枚）または動画を添付",
        "Attach pictures (up to {})" => "画像を添付（最大 {} 枚）",
        "enter opens the folder" => "enter でフォルダーを開く",
        "no folders, pictures, or videos here" => "フォルダー・画像・動画はありません",
        "enter choose  space mark  h up  . hidden  esc cancel" => {
            "enter 選択  space マーク  h 親へ  . 隠しファイル  esc 中止"
        }
        _ => value,
    }
}

pub(super) const ACTIONS: &[(char, &str, &str)] = &[
    ('r', "Reply", "返信"),
    ('Q', "Quote", "引用"),
    ('l', "Like / undo", "リアクション / 取消"),
    ('b', "Repost / undo", "リポスト / 取消"),
    ('f', "Follow / unfollow", "フォロー / 解除"),
    ('M', "Mute / unmute", "ミュート / 解除"),
    ('D', "Deletion request", "削除要求"),
    ('!', "Public report", "公開通報"),
    ('d', "Save attachments", "添付を保存"),
];
