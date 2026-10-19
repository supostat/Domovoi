use domovoi_index::SafetyLevel::{CheckYourself, Recoverable, Safe};

use crate::config_overrides::ConfigOverrides;
use crate::rule::{Condition, Rule};

const XCODE: &str = "com.apple.dt.Xcode";
const SIMULATOR: &str = "com.apple.iphonesimulator";

pub fn rule_table(overrides: &ConfigOverrides) -> Vec<Rule> {
    let mut cargo_target = Rule::new(
        "cargo.target",
        Safe,
        &["**/target|Cargo.toml"],
        "Результаты сборки Rust-проекта. cargo соберёт их заново, исходники не пострадают. \
         Учтены стандартный путь и target-dir из ~/.cargo/config.toml; путь из переменных окружения shell программа не видит.",
    );
    if let Some(target) = &overrides.cargo_target {
        cargo_target = cargo_target.also_at(target);
    }
    let mut pnpm_store = Rule::new(
        "cache.pnpm-store",
        Recoverable,
        &["~/Library/pnpm/store"],
        "Хранилище пакетов pnpm. Проекты сохранят свои копии, но следующая установка скачает пакеты заново. \
         Учтены стандартный путь и store-dir из ~/.config/pnpm/rc; путь из переменных окружения shell программа не видит.",
    );
    if let Some(store) = &overrides.pnpm_store {
        pnpm_store = pnpm_store.also_at(store);
    }
    vec![
        Rule::new(
            "xcode.derived-data",
            Safe,
            &["~/Library/Developer/Xcode/DerivedData"],
            "Промежуточные результаты сборки Xcode. Пересоздаются при следующей сборке, проекты не пострадают.",
        )
        .owned_by(XCODE),
        Rule::new(
            "xcode.device-support",
            Recoverable,
            &[
                "~/Library/Developer/Xcode/iOS DeviceSupport",
                "~/Library/Developer/Xcode/watchOS DeviceSupport",
                "~/Library/Developer/Xcode/tvOS DeviceSupport",
                "~/Library/Developer/Xcode/macOS DeviceSupport",
            ],
            "Символы отладки для подключавшихся устройств. Вернутся только при следующем подключении устройства.",
        )
        .owned_by(XCODE),
        Rule::new(
            "xcode.simulators-unavailable",
            Safe,
            &["~/Library/Developer/CoreSimulator/Devices/*"],
            "Симулятор версии системы, которой больше нет в Xcode. Запустить его уже нельзя.",
        )
        .when(Condition::SimulatorRuntimeMissing)
        .owned_by(SIMULATOR),
        Rule::new(
            "xcode.simulators",
            CheckYourself,
            &["~/Library/Developer/CoreSimulator/Devices/*"],
            "Симулятор Xcode. Внутри живут установленные в нём приложения и их данные.",
        )
        .owned_by(SIMULATOR),
        Rule::new(
            "xcode.dyld-cache",
            Safe,
            &["~/Library/Developer/CoreSimulator/Caches/dyld"],
            "Кэш системных библиотек симуляторов. Пересоздаётся при следующем запуске симулятора.",
        )
        .owned_by(SIMULATOR),
        Rule::new(
            "xcode.caches",
            Safe,
            &["~/Library/Caches/com.apple.dt.Xcode"],
            "Кэш Xcode. Пересоздаётся при следующем запуске.",
        )
        .owned_by(XCODE),
        Rule::new(
            "xcode.archives",
            CheckYourself,
            &["~/Library/Developer/Xcode/Archives"],
            "Архивы сборок Xcode. Нужны, чтобы разбирать отчёты о сбоях уже выпущенных версий.",
        ),
        Rule::new(
            "xcode.simulator-runtimes",
            Recoverable,
            &[
                "/Library/Developer/CoreSimulator/Volumes/*",
                "/Library/Developer/CoreSimulator/Images/*",
            ],
            "Образы систем для симуляторов. Это системные файлы: удаляет их только сам Xcode.",
        )
        .info("xcrun simctl runtime delete <идентификатор>"),
        Rule::new(
            "docker.vm",
            CheckYourself,
            &["~/Library/Containers/com.docker.docker/Data/vms"],
            "Диск виртуальной машины Docker. В нём образы, контейнеры и тома: удаление сотрёт их.",
        )
        .owned_by("com.docker.docker"),
        Rule::new(
            "orbstack",
            CheckYourself,
            &["~/.orbstack"],
            "Данные OrbStack: машины, контейнеры и тома. Удаление сотрёт их.",
        )
        .owned_by("dev.kdrag0n.MacVirt"),
        Rule::new(
            "ios.backups",
            CheckYourself,
            &["~/Library/Application Support/MobileSync/Backup/*"],
            "Резервная копия iPhone или iPad. Возможно, других копий этих данных нет.",
        ),
        cargo_target,
        Rule::new(
            "dev.deps",
            Safe,
            &[
                "**/node_modules|package.json",
                "**/zig-cache|build.zig",
                "**/.zig-cache|build.zig",
            ],
            "Зависимости и кэш сборки проекта. Вернутся после установки зависимостей и сборки.",
        ),
        Rule::new(
            "downloads.installers",
            Recoverable,
            &["~/Downloads/*.dmg", "~/Downloads/*.pkg"],
            "Установщик из Загрузок. Если программа понадобится снова, его придётся скачать заново.",
        ),
        Rule::new(
            "cache.homebrew",
            Safe,
            &["~/Library/Caches/Homebrew"],
            "Скачанные Homebrew архивы. Установленные программы не пострадают.",
        ),
        Rule::new(
            "cache.npm",
            Safe,
            &["~/.npm/_cacache"],
            "Кэш npm. Пакеты скачаются заново при следующей установке.",
        ),
        pnpm_store,
        Rule::new(
            "cache.pnpm-metadata",
            Safe,
            &["~/Library/Caches/pnpm"],
            "Кэш метаданных pnpm. Пересоздаётся при следующей установке.",
        ),
        Rule::new(
            "cache.yarn",
            Safe,
            &["~/Library/Caches/Yarn"],
            "Кэш Yarn. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.yarn-berry",
            Safe,
            &["~/.yarn/berry/cache"],
            "Общий кэш Yarn Berry. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.bun",
            Safe,
            &["~/.bun/install/cache"],
            "Кэш Bun. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.cargo-registry",
            Recoverable,
            &["~/.cargo/registry"],
            "Скачанные пакеты crates.io. cargo скачает их заново; без сети сборка не пройдёт.",
        ),
        Rule::new(
            "cache.cargo-git",
            Recoverable,
            &["~/.cargo/git"],
            "Зависимости cargo из git-репозиториев. Скачаются заново; без сети сборка не пройдёт.",
        ),
        Rule::new(
            "cache.pip",
            Safe,
            &["~/Library/Caches/pip"],
            "Кэш pip. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.uv",
            Safe,
            &["~/.cache/uv"],
            "Кэш uv. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.poetry",
            Safe,
            &["~/Library/Caches/pypoetry/cache", "~/Library/Caches/pypoetry/artifacts"],
            "Кэш Poetry. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.poetry-venvs",
            Recoverable,
            &["~/Library/Caches/pypoetry/virtualenvs"],
            "Виртуальные окружения Poetry. Пересоздаются командой poetry install.",
        ),
        Rule::new(
            "cache.gradle",
            Safe,
            &["~/.gradle/caches"],
            "Кэш Gradle. Зависимости скачаются при следующей сборке.",
        ),
        Rule::new(
            "cache.maven",
            Recoverable,
            &["~/.m2/repository"],
            "Локальный репозиторий Maven. Зависимости скачаются заново; без сети сборка не пройдёт.",
        ),
        Rule::new(
            "cache.cocoapods",
            Safe,
            &["~/Library/Caches/CocoaPods"],
            "Кэш CocoaPods. Пересоздаётся при следующей установке.",
        ),
        Rule::new(
            "cache.cocoapods-specs",
            Recoverable,
            &["~/.cocoapods/repos"],
            "Каталог спецификаций CocoaPods. Скачается заново при следующем pod install.",
        ),
        Rule::new(
            "cache.swiftpm",
            Safe,
            &["~/Library/Caches/org.swift.swiftpm"],
            "Кэш Swift Package Manager. Пакеты скачаются при следующей сборке.",
        ),
        Rule::new(
            "cache.go-build",
            Safe,
            &["~/Library/Caches/go-build"],
            "Кэш сборки Go. Пересоздаётся при следующей сборке.",
        ),
        Rule::new(
            "cache.go-mod",
            Recoverable,
            &["~/go/pkg/mod"],
            "Скачанные модули Go. Скачаются заново; без сети сборка не пройдёт.",
        ),
        Rule::new(
            "cache.composer",
            Safe,
            &["~/Library/Caches/composer", "~/.composer/cache"],
            "Кэш Composer. Пакеты скачаются заново при следующей установке.",
        ),
        Rule::new(
            "cache.nuget-http",
            Safe,
            &["~/.local/share/NuGet/http-cache"],
            "HTTP-кэш NuGet. Пересоздаётся при следующей установке.",
        ),
        Rule::new(
            "cache.nuget-packages",
            Recoverable,
            &["~/.nuget/packages"],
            "Скачанные пакеты NuGet. Скачаются заново; без сети сборка не пройдёт.",
        ),
        Rule::new(
            "cache.carthage",
            Safe,
            &["~/Library/Caches/org.carthage.CarthageKit"],
            "Кэш Carthage. Пересоздаётся при следующей сборке.",
        ),
        Rule::new(
            "cache.chrome",
            Safe,
            &["~/Library/Caches/Google/Chrome"],
            "Кэш Google Chrome. Страницы загрузятся заново, закладки и пароли не пострадают.",
        )
        .owned_by("com.google.Chrome"),
        Rule::new(
            "cache.slack",
            Safe,
            &[
                "~/Library/Application Support/Slack/Cache",
                "~/Library/Application Support/Slack/Code Cache",
                "~/Library/Application Support/Slack/Service Worker/CacheStorage",
            ],
            "Кэш Slack. Переписка хранится на сервере и загрузится заново.",
        )
        .owned_by("com.tinyspeck.slackmacgap"),
        Rule::new(
            "cache.app.generic",
            Safe,
            &["~/Library/Containers/*/Data/Library/Caches"],
            "Кэш приложения из App Store. Приложение создаст его заново.",
        ),
        Rule::new(
            "caches.user",
            Safe,
            &["~/Library/Caches/*"],
            "Кэш программы. Она создаст его заново, данные не пострадают.",
        ),
    ]
}
