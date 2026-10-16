# DOMOVOI

Анализатор диска для macOS: показывает, где лежит занятое место, рекомендует, что можно убрать, и отвечает на вопросы о диске с помощью локальной языковой модели. Весь код на Rust, интерфейс на AppKit через objc2, диаграмма в Metal.

Продукт и архитектура — [docs/DOMOVOI.md](docs/DOMOVOI.md). Внешний вид — [design/domovoi-design-book.html](design/domovoi-design-book.html).

## Требования

- Mac на Apple Silicon (arm64), macOS 15 или новее.
- Xcode 26.1 SDK и Metal Toolchain: `xcodebuild -downloadComponent metalToolchain`.
- Rust 1.94.0 — ставится сам по `rust-toolchain.toml`.
- cmake 3.17 или новее.
- Python 3.10 или новее — только для скриптов в `research/` (генерация тестовой GGUF и golden-промптов); в CI не нужен, результаты закоммичены.

## Сборка и проверки

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Команды xtask

| Команда | Что делает |
|---|---|
| `cargo xtask check-comments` | Падает, если комментарий в `crates/` ссылается на книгу, фазы или спеку. |
| `cargo xtask check-tokens` | Падает на hex-цветах и `rgba(` вне крейтов `theme` и `xtask`. |
| `cargo xtask check-sources` | Падает, если документ или книга расходятся с принятыми решениями. |

## Профили подписи

| Профиль | Подпись | Для чего |
|---|---|---|
| `dev` | Developer ID, hardened runtime | Ежедневная разработка. TCC запоминает идентичность приложения, поэтому Full Disk Access переживает пересборки. |
| `adhoc` | ad-hoc (`-`), без hardened runtime | CI без сертификата. Фича `adhoc` отключает проверку подписи собеседника в XPC. |
| `release` | Developer ID, hardened runtime, нотаризация | Выпуск через Sparkle. |

## Структура

```
crates/
  apfs-sys         биндинги и безопасные обёртки над APFS, TCC-errno, libproc
  testkit          генератор фикстурных деревьев и APFS-образов
  scanner          обход Data-тома, классификация доступа, FSEvents
  index            дерево, агрегаты, давность, числа тома, кэш скана
  rules            правила, denylist, рекомендации, Корзина Домового
  dups             дубликаты и замена клоном
  theme            токены дизайна, движение, форматирование
  render           Metal-диаграмма
  grammar          схемы ответа, JSON-схема → GBNF, шаблон промпта
  llama-sys        llama.cpp на пине и биндинги
  xpc-sys          биндинги libxpc
  inference-proto  протокол сервиса инференса
  inference        XPC-сервис инференса
  downloader       каталог моделей и загрузчик
  agent            агентный цикл и инструменты
  eval             эвал моделей каталога
  app              приложение
  xtask            сборочные задачи и проверки
```
