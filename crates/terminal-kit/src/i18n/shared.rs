//! Vocabulary shared by the clients.
use super::{Lang, Table};

const JA: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter で検索 · Ctrl+T 投稿/アカウント · Esc キャンセル",
    ),
    ("Enter search · Esc cancel", "Enter 検索 · Esc キャンセル"),
    ("Esc back to timeline", "Esc タイムラインに戻る"),
    ("Help", "ヘルプ"),
    ("Language", "言語"),
    ("No results for \"{}\"", "「{}」の検索結果はありません"),
    ("Search", "検索"),
    ("Search: {}", "検索: {}"),
    ("Settings", "設定"),
    ("Terminal too small", "端末が小さすぎます"),
    ("Theme", "テーマ"),
    ("Thread", "スレッド"),
    ("attach", "添付"),
    ("move the selection", "選択を移動"),
    ("open", "開く"),
    ("quit", "終了"),
];

const ZH: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter 搜索 · Ctrl+T 帖子/账号 · Esc 取消",
    ),
    ("Enter search · Esc cancel", "Enter 搜索 · Esc 取消"),
    ("Esc back to timeline", "Esc 返回时间线"),
    ("Help", "帮助"),
    ("Language", "语言"),
    ("No results for \"{}\"", "没有“{}”的搜索结果"),
    ("Search", "搜索"),
    ("Search: {}", "搜索：{}"),
    ("Settings", "设置"),
    ("Terminal too small", "终端窗口太小"),
    ("Theme", "主题"),
    ("Thread", "帖子串"),
    ("attach", "附加"),
    ("move the selection", "移动选择"),
    ("open", "打开"),
    ("quit", "退出"),
];

const KO: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter 검색 · Ctrl+T 게시물/계정 · Esc 취소",
    ),
    ("Enter search · Esc cancel", "Enter 검색 · Esc 취소"),
    ("Esc back to timeline", "Esc 타임라인으로 돌아가기"),
    ("Help", "도움말"),
    ("Language", "언어"),
    ("No results for \"{}\"", "“{}” 검색 결과가 없습니다"),
    ("Search", "검색"),
    ("Search: {}", "검색: {}"),
    ("Settings", "설정"),
    ("Terminal too small", "터미널이 너무 작습니다"),
    ("Theme", "테마"),
    ("Thread", "스레드"),
    ("attach", "첨부"),
    ("move the selection", "선택 이동"),
    ("open", "열기"),
    ("quit", "종료"),
];

const RU: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter поиск · Ctrl+T посты/аккаунты · Esc отмена",
    ),
    ("Enter search · Esc cancel", "Enter поиск · Esc отмена"),
    ("Esc back to timeline", "Esc назад к ленте"),
    ("Help", "Справка"),
    ("Language", "Язык"),
    ("No results for \"{}\"", "Нет результатов для «{}»"),
    ("Search", "Поиск"),
    ("Search: {}", "Поиск: {}"),
    ("Settings", "Настройки"),
    ("Terminal too small", "Терминал слишком мал"),
    ("Theme", "Тема"),
    ("Thread", "Ветка"),
    ("attach", "прикрепить"),
    ("move the selection", "двигать выделение"),
    ("open", "открыть"),
    ("quit", "выход"),
];

const ES: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter buscar · Ctrl+T publicaciones/cuentas · Esc cancelar",
    ),
    ("Enter search · Esc cancel", "Enter buscar · Esc cancelar"),
    ("Esc back to timeline", "Esc volver a la cronología"),
    ("Help", "Ayuda"),
    ("Language", "Idioma"),
    ("No results for \"{}\"", "No hay resultados para «{}»"),
    ("Search", "Buscar"),
    ("Search: {}", "Búsqueda: {}"),
    ("Settings", "Ajustes"),
    ("Terminal too small", "Terminal demasiado pequeña"),
    ("Theme", "Tema"),
    ("Thread", "Hilo"),
    ("attach", "adjuntar"),
    ("move the selection", "mover la selección"),
    ("open", "abrir"),
    ("quit", "salir"),
];

const FR: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Entrée rechercher · Ctrl+T posts/comptes · Échap annuler",
    ),
    (
        "Enter search · Esc cancel",
        "Enter rechercher · Esc annuler",
    ),
    ("Esc back to timeline", "Échap retour au fil"),
    ("Help", "Aide"),
    ("Language", "Langue"),
    ("No results for \"{}\"", "Aucun résultat pour « {} »"),
    ("Search", "Rechercher"),
    ("Search: {}", "Recherche : {}"),
    ("Settings", "Réglages"),
    ("Terminal too small", "Terminal trop petit"),
    ("Theme", "Thème"),
    ("Thread", "Fil"),
    ("attach", "joindre"),
    ("move the selection", "déplacer la sélection"),
    ("open", "ouvrir"),
    ("quit", "quitter"),
];

const DE: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter suchen · Strg+T Beiträge/Konten · Esc abbrechen",
    ),
    ("Enter search · Esc cancel", "Enter suchen · Esc abbrechen"),
    ("Esc back to timeline", "Esc zurück zur Timeline"),
    ("Help", "Hilfe"),
    ("Language", "Sprache"),
    ("No results for \"{}\"", "Keine Ergebnisse für „{}“"),
    ("Search", "Suche"),
    ("Search: {}", "Suche: {}"),
    ("Settings", "Einstellungen"),
    ("Terminal too small", "Terminal zu klein"),
    ("Theme", "Design"),
    ("Thread", "Thread"),
    ("attach", "anhängen"),
    ("move the selection", "Auswahl bewegen"),
    ("open", "öffnen"),
    ("quit", "beenden"),
];

const PT: Table = &[
    (
        "Enter search · Ctrl+T posts/accounts · Esc cancel",
        "Enter buscar · Ctrl+T posts/contas · Esc cancelar",
    ),
    (
        "Enter search · Esc cancel",
        "Enter pesquisar · Esc cancelar",
    ),
    ("Esc back to timeline", "Esc voltar à timeline"),
    ("Help", "Ajuda"),
    ("Language", "Idioma"),
    ("No results for \"{}\"", "Nenhum resultado para \"{}\""),
    ("Search", "Buscar"),
    ("Search: {}", "Busca: {}"),
    ("Settings", "Configurações"),
    ("Terminal too small", "Terminal pequeno demais"),
    ("Theme", "Tema"),
    ("Thread", "Fio"),
    ("attach", "anexar"),
    ("move the selection", "mover a seleção"),
    ("open", "abrir"),
    ("quit", "sair"),
];

pub(super) fn table(lang: Lang) -> Table {
    match lang {
        Lang::En => &[],
        Lang::Ja => JA,
        Lang::Zh => ZH,
        Lang::Ko => KO,
        Lang::Ru => RU,
        Lang::Es => ES,
        Lang::Fr => FR,
        Lang::De => DE,
        Lang::Pt => PT,
    }
}
