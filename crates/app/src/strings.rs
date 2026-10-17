use domovoi_render::DiagramStrings;

pub const APP_NAME: &str = "DOMOVOI";
pub const DISK_GROUP: &str = "Диск";
pub const ORDER_GROUP: &str = "Порядок";
pub const SETTINGS_GROUP: &str = "Настройки";
pub const RECOMMENDATIONS: &str = "Рекомендации";
pub const DUPLICATES: &str = "Дубликаты";
pub const ASSISTANT: &str = "Домовой";
pub const ASSISTANT_BADGE: &str = "AI";
pub const MODEL: &str = "Модель";
pub const FREE: &str = "свободно";
pub const OF: &str = "из";
pub const ACCESS_TITLE: &str = "Домовому нужен доступ ко всему диску";
pub const ACCESS_BODY: &str = "Иначе macOS скроет от него папки Library, почту и данные приложений, и часть места останется неучтённой. Файлы не открываются и никуда не отправляются.";
pub const ACCESS_STEPS: [[&str; 3]; 3] = [
    [
        "Откройте ",
        "Настройки → Конфиденциальность → Полный доступ к диску",
        ".",
    ],
    ["Включите ", "DOMOVOI", " в списке."],
    ["Вернитесь сюда, доступ проверится сам.", "", ""],
];
pub const OPEN_SETTINGS: &str = "Открыть Настройки";
pub const SCAN_WITHOUT_ACCESS: &str = "Сканировать без доступа";
pub const RELAUNCH: &str = "Перезапустить";
pub const RELAUNCH_HINT: &str = "Доступ выдан. macOS увидит его после перезапуска.";
pub const SCANNING: &str = "Сканирование";
pub const STOP: &str = "Остановить";
pub const SCAN_ENGINE: &str = "getattrlistbulk";
pub const THREADS_ONE: &str = "поток";
pub const THREADS_FEW: &str = "потока";
pub const THREADS_MANY: &str = "потоков";
pub const WALKED: &str = "обойдено";
pub const FULL_FIRST_SCAN: &str = "После обновления macOS первый скан полный.";
pub const FILES_MILLIONS: &str = "млн файлов";
pub const TRAY: &str = "Корзина Домового";
pub const SHOW_IN_FINDER: &str = "Показать в Finder";
pub const SUNBURST: &str = "Sunburst";
pub const TREEMAP: &str = "Treemap";
pub const SOURCE_SCAN: &str = "по скану";
pub const SOURCE_APFS: &str = "по данным APFS";
pub const OTHER_VOLUMES: &str = "Другие тома";
pub const MACOS: &str = "macOS";
pub const IN_CLOUD: &str = "в облаке";
pub const OUTSIDE_TOTAL: &str = "вне суммы";
pub const SOURCE_MACOS_ESTIMATE: &str = "оценка macOS";
pub const DATA_VOLUME: &str = "Данные";
pub const PURGEABLE: &str = "Purgeable";
pub const SYSTEM_DATA: &str = "Системные данные";
pub const HIDE_APP: &str = "Скрыть DOMOVOI";
pub const QUIT_APP: &str = "Завершить DOMOVOI";
pub const WINDOW_MENU: &str = "Окно";
pub const MINIMIZE: &str = "Свернуть";
pub const CLOSE_WINDOW: &str = "Закрыть окно";
pub const FAILURE_TITLE: &str = "Домовой не смог продолжить";
pub const FAILURE_CLOSE: &str = "Закрыть";
pub const FDA_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_AllFiles";

pub const DIAGRAM: DiagramStrings = DiagramStrings {
    category_names: [
        "Приложения",
        "Разработка",
        "Медиа",
        "Документы",
        "Кэши",
        "Система",
        "Скрытое",
        "Модель",
        "Дом",
    ],
    used: "занято",
    up: "↑ наверх",
};
