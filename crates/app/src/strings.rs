use domovoi_render::DiagramStrings;
use domovoi_theme::PluralForms;

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
pub const THREADS: PluralForms = PluralForms {
    one: "поток",
    few: "потока",
    many: "потоков",
};
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
pub const HIDDEN_BY_MACOS: &str = "macOS скрыла";
pub const HIDDEN_FOLDERS: PluralForms = PluralForms {
    one: "папку",
    few: "папки",
    many: "папок",
};
pub const HIDDEN_FOLDERS_HINT: &str = "Дайте полный доступ к диску, чтобы учесть их";
pub const SNAPSHOTS_RELEASE: &str = "macOS удалит снапшоты сама, когда место понадобится";

pub const CAN_FREE: &str = "Можно освободить";
pub const LEVEL_SAFE: &str = "Безопасно";
pub const LEVEL_RECOVERABLE: &str = "Восстановимо";
pub const LEVEL_CHECK_YOURSELF: &str = "Проверьте сами";
pub const SPLIT_SAFE: &str = "безопасно";
pub const SPLIT_RECOVERABLE: &str = "восстановимо";
pub const SPLIT_CHECK_YOURSELF: &str = "проверьте";
pub const TO_TRAY: &str = "В Корзину Домового";
pub const RULE_PROVENANCE: &str = "правило";
pub const MODEL_HYPOTHESIS: &str = "гипотеза модели";
pub const HELD_PREFIX: &str = "Останется занято:";
pub const HELD_BY_SNAPSHOTS: &str = "держат снапшоты";
pub const HELD_BY_CLONES: &str = "держат клоны";
pub const HELD_BY_HARD_LINKS: &str = "держат жёсткие ссылки";
pub const CLOSE_OWNER: &str = "Закройте";
pub const CLOSE_OWNER_SUFFIX: &str = "чтобы убрать эти файлы";
pub const VENDOR_COMMAND: &str = "Удаляется командой";
pub const PATHS: PluralForms = PluralForms {
    one: "путь",
    few: "пути",
    many: "путей",
};
pub const NOT_TOUCHED: &str = "не трогаем";
pub const DENY_PROTECTED_FLAG: &str = "защищено системой";
pub const DENY_SYSTEM_VOLUME: &str = "системный том";
pub const DENY_OTHER_VOLUME: &str = "другой том";
pub const DENY_FIRMLINK: &str = "системная ссылка";
pub const DENY_PARENT_NOT_WRITABLE: &str = "нет прав на папку";
pub const DENY_PROTECTED_DATA: &str = "данные под защитой";
pub const DENY_RUNNING_APPLICATION: &str = "приложение запущено";
pub const DENY_OWN_APPLICATION: &str = "это сам Домовой";
pub const DENY_TRASH: &str = "уже в Корзине";
pub const NO_RECOMMENDATIONS: &str = "Правилам здесь нечего предложить. Диск выглядит аккуратно.";

pub const FIND_DUPLICATES: &str = "Искать дубликаты";
pub const DUPLICATES_IDLE: &str = "Дубликаты ищутся отдельным проходом: размер, края файла, затем полное содержимое.";
pub const GROUPS: PluralForms = PluralForms {
    one: "группа",
    few: "группы",
    many: "групп",
};
pub const WILL_FREE: &str = "освободится";
pub const CHECKED: &str = "проверено";
pub const CANDIDATES: PluralForms = PluralForms {
    one: "кандидат",
    few: "кандидата",
    many: "кандидатов",
};
pub const MINUTES_SHORT: &str = "мин";
pub const SECONDS_SHORT: &str = "с";
pub const ACTION_TRASH: &str = "В Корзину";
pub const ACTION_CLONE: &str = "Заменить клоном";
pub const CLONE_EXPLANATION: &str = "Имена остаются, место общее. Изменение любой копии снова занимает место.";
pub const DUPLICATES_NOTE: &str = "По умолчанию копии уходят в Корзину. «Заменить клоном» — явное действие для группы: доступно, когда копия одна на своём иноде, принадлежит вам, не открыта и не лежит в пакете или облачной папке.";
pub const OTHER_USERS_FILES: &str = "Открытые файлы других пользователей не видны — закройте их сами.";
pub const KEEP: &str = "оставить";
pub const APFS_CLONE: &str = "клон APFS";
pub const COPIES: PluralForms = PluralForms {
    one: "копия",
    few: "копии",
    many: "копий",
};
pub const EACH: &str = "каждая";
pub const FREES: &str = "освободит";
pub const STAGE_EDGES: &str = "Сравниваю размеры и края файлов";
pub const STAGE_CONTENTS: &str = "Сравниваю содержимое";
pub const STAGE_CLOUD: &str = "Проверяю облачные папки";
pub const NO_DUPLICATES: &str = "Одинаковых файлов не нашлось.";
pub const REFUSED_NOT_REGULAR: &str = "не обычный файл";
pub const REFUSED_OTHER_VOLUME: &str = "копии на разных томах";
pub const REFUSED_VOLUME_CANNOT_CLONE: &str = "том не умеет клоны";
pub const REFUSED_VOLUME_CANNOT_SWAP: &str = "том не умеет подмену";
pub const REFUSED_HARD_LINKED: &str = "у копии есть жёсткие ссылки";
pub const REFUSED_NOT_OWNED: &str = "копия принадлежит другому пользователю";
pub const REFUSED_NOT_WRITABLE: &str = "нет прав на запись";
pub const REFUSED_PROTECTED: &str = "файл защищён флагами";
pub const REFUSED_PURGEABLE: &str = "macOS может удалить файл сама";
pub const REFUSED_BUSY: &str = "файл занят";
pub const REFUSED_DENIED: &str = "путь закрыт для изменений";
pub const REFUSED_PACKAGE: &str = "файл внутри пакета";
pub const REFUSED_SYNCED: &str = "файл в облачной папке";
pub const REFUSED_OPEN: &str = "файл открыт в другом приложении";
pub const REFUSED_ALREADY_CLONES: &str = "уже клоны";
pub const REFUSED_CHANGED: &str = "содержимое изменилось";
pub const REFUSED_UNREADABLE: &str = "файл не читается";

pub const WILL_MOVE: &str = "Будет перемещено в Корзину";
pub const TRAY_NOTE: &str =
    "Файлы можно вернуть из Корзины Finder, пока она не очищена. Место освободится после очистки Корзины.";
pub const CANCEL: &str = "Отмена";
pub const MOVE_TO_TRASH: &str = "Переместить в Корзину";
pub const MOVED_TO_TRASH: &str = "Перемещено в Корзину:";
pub const FREED_AFTER_EMPTYING: &str = "Место освободится после очистки Корзины.";
pub const OPEN_TRASH: &str = "Открыть Корзину";
pub const CLOSE: &str = "Закрыть";
pub const RECENTLY_MOVED: &str = "Недавно перемещённые";
pub const RESTORE: &str = "Вернуть";
pub const NOT_MOVED: &str = "Не перемещено:";
pub const TRAY_EMPTY: &str =
    "Корзина Домового пуста. Отметьте рекомендации или копии дубликатов, и они соберутся здесь.";
pub const TRASH_NO_PERMISSION: &str = "нет прав";
pub const TRASH_READ_ONLY: &str = "том только для чтения";
pub const TRASH_MISSING: &str = "файла уже нет";
pub const TRASH_NO_TRASH: &str = "у тома нет Корзины";
pub const TRASH_OUT_OF_SPACE: &str = "не хватило места";
pub const TRASH_OTHER: &str = "macOS отказала";
pub const RESTORE_NOT_IN_TRASH: &str = "Файла уже нет в Корзине";
pub const RESTORE_REPLACED: &str = "В Корзине на его месте уже другой файл";
pub const RESTORE_PARENT_MISSING: &str = "Исходной папки больше нет";
pub const RESTORE_FAILED: &str = "Не удалось вернуть";
pub const CLONED: &str = "Заменено клоном:";
pub const NOT_CLONED: &str = "Не заменено:";

pub const RULE_TITLES: [(&str, &str); 42] = [
    ("xcode.derived-data", "Xcode · DerivedData"),
    ("xcode.device-support", "iOS DeviceSupport"),
    ("xcode.simulators", "Симуляторы Xcode"),
    ("xcode.simulators-unavailable", "Симуляторы удалённых версий iOS"),
    ("xcode.simulator-runtimes", "Образы систем для симуляторов"),
    ("xcode.dyld-cache", "Кэш библиотек симуляторов"),
    ("xcode.caches", "Кэш Xcode"),
    ("xcode.archives", "Архивы Xcode"),
    ("cargo.target", "Каталоги target/ (Rust)"),
    ("dev.deps", "node_modules и zig-cache"),
    ("caches.user", "Кэши приложений"),
    ("cache.app.generic", "Кэши приложений из App Store"),
    ("cache.bun", "Кэш bun"),
    ("cache.cargo-git", "Кэш git-зависимостей cargo"),
    ("cache.cargo-registry", "Кэш реестра cargo"),
    ("cache.carthage", "Кэш Carthage"),
    ("cache.chrome", "Кэш Google Chrome"),
    ("cache.cocoapods", "Кэш CocoaPods"),
    ("cache.cocoapods-specs", "Спецификации CocoaPods"),
    ("cache.composer", "Кэш Composer"),
    ("cache.go-build", "Кэш сборки Go"),
    ("cache.go-mod", "Модули Go"),
    ("cache.gradle", "Кэш Gradle"),
    ("cache.homebrew", "Кэш Homebrew"),
    ("cache.maven", "Репозиторий Maven"),
    ("cache.npm", "Кэш npm"),
    ("cache.nuget-http", "HTTP-кэш NuGet"),
    ("cache.nuget-packages", "Пакеты NuGet"),
    ("cache.pip", "Кэш pip"),
    ("cache.pnpm-metadata", "Метаданные pnpm"),
    ("cache.pnpm-store", "Хранилище pnpm"),
    ("cache.poetry", "Кэш Poetry"),
    ("cache.poetry-venvs", "Окружения Poetry"),
    ("cache.slack", "Кэш Slack"),
    ("cache.swiftpm", "Кэш SwiftPM"),
    ("cache.uv", "Кэш uv"),
    ("cache.yarn", "Кэш Yarn"),
    ("cache.yarn-berry", "Кэш Yarn Berry"),
    ("docker.vm", "Диск Docker"),
    ("orbstack", "Данные OrbStack"),
    ("downloads.installers", "Установщики в Загрузках"),
    ("ios.backups", "Резервные копии iPhone и iPad"),
];

pub fn rule_title(id: &str) -> Option<&'static str> {
    RULE_TITLES
        .iter()
        .find(|(rule, _)| *rule == id)
        .map(|(_, title)| *title)
}

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
