//! Translations into Japanese, by the English they translate.

pub const TABLE: &[(&str, &str)] = &[
    ("(opened in the web browser)", "(ブラウザで開きました)"),
    (
        "mstdn is logged in. You can close this tab and go back to the terminal.",
        "mstdn にログインしました。このタブを閉じて端末に戻ってください。",
    ),
    ("Nothing here.", "ここには何もありません。"),
    (
        "Paste the code the page shows, then press Enter: ",
        "ページに表示されたコードを貼り付けて Enter を押してください: ",
    ),
    (
        "The access was not given. You can close this tab.",
        "アクセスは許可されませんでした。このタブは閉じて構いません。",
    ),
    (
        "The server sent no code.",
        "サーバーからコードが届きませんでした。",
    ),
    (
        "This is not the login mstdn is waiting for.",
        "これは mstdn が待っているログインではありません。",
    ),
    (
        "To log in on {}, open this address, signed in there, and allow mstdn to read and write:",
        "{} にログインするには、そのサーバーにサインインした状態でこのアドレスを開き、mstdn に読み書きを許可してください:",
    ),
    (
        "Waiting for the browser to come back (up to {} s; Ctrl-C to give up)...",
        "ブラウザから戻るのを待っています (最大 {} 秒。Ctrl-C でやめます)...",
    ),
];
