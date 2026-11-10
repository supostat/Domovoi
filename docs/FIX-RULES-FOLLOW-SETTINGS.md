<!--
  FIX-RULES-FOLLOW-SETTINGS.md — дельта к docs/SPEC-DOMOVOI.md: среды симуляторов, установленные при открытом
  Домовом, и правка ~/.cargo/config.toml не видны в рекомендациях до перезапуска. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1 из трёх, с правкой на ревью): RuleSet со всем окружением (настройки
  cargo и pnpm, папка снимков, установленные среды симуляторов) собирается один раз в Session::open и не обновляется
  даже при пересканировании; правка — правила собираются заново при каждом скане и при дельте, задевшей папку-источник
  или её прямого ребёнка (точку монтирования среды).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-SIMULATOR-DEVICE-SET.md` (`04834ca` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Среда симулятора, установленная при открытом Домовом, не снимает с её устройств пометку «запустить уже
  нельзя» и уровень «Безопасно»; правка `~/.cargo/config.toml` не меняет карточку `cargo.target` — до перезапуска,
  даже после «Пересканировать».
- Уже есть и не меняется: таблица правил и чтение настроек (`ConfigOverrides::read`, `InstalledSimulatorRuntimes::discover`);
  частичное обновление рекомендаций дельтой, когда источники правил не задеты; список снимков перед сканом и дельтой;
  полный скан после размонтирования тома.
- Не входит: первое появление самой папки-источника (её нет в индексе — изменения подхватит следующий скан); папка
  снимков экрана при дельте (кэш `NSUserDefaults` в процессе не проверен) — она перечитывается при скане.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/rules/src/config_overrides.rs`: `ConfigOverrides::source_folders(home)` — `~/.cargo` и `~/.config/pnpm`
  (папки `CARGO_CONFIG` и `PNPM_RC`).
- `crates/rules/src/simulator_runtimes.rs`: `InstalledSimulatorRuntimes::source_folders()` — папки сред
  `/Library/Developer/CoreSimulator/Profiles/Runtimes` и `/Library/Developer/CoreSimulator/Volumes`.
- `crates/app/src/pending_delta.rs`: поле `touches_rule_sources: bool`.
- `crates/app/src/session.rs`: сборка правил одним местом (`rules_for(root, home)` — то, что сейчас в `open`);
  `scan` собирает правила заново до `install`; `delta_request` ставит `touches_rule_sources`, когда путь события
  `Changed` — папка-источник или её прямой ребёнок (сопоставление через `index.find` пути или его родителя: новой
  точки монтирования в индексе ещё нет), либо событие пересканирует поддерево, в котором лежит источник;
  `apply_listings` с этим признаком собирает правила заново и строит рекомендации полностью
  (`refresh_recommendations`) вместо `apply_delta`.
- `crates/app/tests/rules_follow_settings.rs` (написан диагнозом): регрессия — скан, дельта по папке-источнику, дельта
  по новой папке внутри неё.
- `docs/DOMOVOI.md`, раздел 4.4: пути правил зависят от окружения (`target-dir` из `~/.cargo/config.toml`,
  `store-dir` из `~/.config/pnpm/rc`, установленные среды симуляторов); они перечитываются при каждом скане и когда
  меняется папка источника или в ней появляется новая папка — перезапуск не нужен.

# Conventions

- Окружение правил обновляется там же, где Session уже обновляет окружение индекса (снимки экрана перед сканом и
  дельтой); путь события сопоставляется через `Index::find`, а не сравнением строк — события Data-тома приходят и
  как `/System/Volumes/Data/…`, и как `/Users/…`.
- Только прямой ребёнок, не всё поддерево: записи сборок в `~/.cargo/registry/…` не должны пересобирать правила.
- Ошибка чтения настроек при пересборке — `SessionError::Io`, как ошибка записи кэша; не проглатывается.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test rules_follow_settings` — 0 passed, 3 failed:
    panicked at crates/app/tests/rules_follow_settings.rs:108:5:
    ["**/target|Cargo.toml", "/Users/igor/builds/target"] follows the target-dir changed without a restart
    panicked at crates/app/tests/rules_follow_settings.rs:121:5: the card follows the edited config without a rescan
    panicked at crates/app/tests/rules_follow_settings.rs:136:5 (новая папка внутри .cargo)
    left: {".../builds/first/target"}
    right: {".../builds/moved/target"}
  Проверка после первого скана (карточка = первая папка target) проходит — красное даёт именно дельта.
- **Причина.** `Session::open` строит `RuleSet::for_volume` из `ConfigOverrides::read(home)` и папки снимков
  (`session.rs:78-81`); `for_volume` зовёт `InstalledSimulatorRuntimes::discover()` (`rule_set.rs:82-93`); после этого
  правила не пересобираются: `scan` (`session.rs:140-158`) и дельта (`apply_delta` с `&self.rules`,
  `session.rs:214-217`) берут прежние.
- **Образец.** Снимки экрана перечитываются перед сканом и дельтой (`session.rs:155`, `session.rs:203`).
- **Цена.** `discover` на этой машине — 2 среды, 2 вызова `plutil`, около 13 мс; полная сборка рекомендаций проверяет
  каждое устройство ещё одним `plutil` (23 устройства). Поэтому пересборка на дельте — только по источникам.
- **События.** Поток FSEvents создаётся без `FileEvents` (`crates/scanner/src/ffi.rs:57`): правка `config.toml`
  приходит событием по папке `.cargo`; корень индекса понимает и `/System/Volumes/Data/…`, и `/Users/…`
  (`crates/index/src/index_root.rs:94-112`).
- **Монтирование среды.** `mount` на этой машине: каждая среда — отдельный APFS-том в
  `/Library/Developer/CoreSimulator/Volumes/<имя>` (`/dev/disk7s1 on .../Volumes/iOS_23B86`). Установка — новая
  точка монтирования и монтирование. Событие по папке `Volumes` (новая точка) может прийти до монтирования, и
  `discover` прочитает пустую папку; событие монтирования приходит после и, по документации Apple к
  `kFSEventStreamEventFlagMount`, несёт путь точки монтирования — его `VolumeEvent::from_raw` отдаёт как `Changed`
  (`crates/scanner/src/volume_events.rs:68-84`). Поэтому прямой ребёнок источника тоже считается. Живым опытом путь
  события монтирования не проверен.
- **Размонтирование.** `Unmount` даёт `EventRequest::RescanRequired` (`session.rs:184`) — полный скан, а с правкой
  скан собирает правила заново: удалённая среда тоже подхватывается.
- **Отвергнуто.** Гипотеза 2 — только при полном скане: без «Пересканировать» правка не видна. Гипотеза 3 — перед
  каждой дельтой со сравнением: `discover` и процессы `plutil` на каждую пачку событий, пока пользователь работает.
  Всё поддерево источника вместо прямого ребёнка: записи `~/.cargo/registry` пересобирали бы правила постоянно.
- **Проверка мутацией.** Одна мутация на прогон: без пересборки в `scan` — краснеет тест скана; без признака
  `touches_rule_sources` — краснеют оба теста дельты; только сама папка без прямого ребёнка — краснеет тест новой
  папки; пересборка правил с `apply_delta` вместо полной сборки рекомендаций — краснеет тест дельты (папки target
  дельта не трогает).

# Gameplan

<!-- 1 фаза: правила следуют за настройками и средами без перезапуска. -->

### Phase 1: rules-follow-settings — рекомендации видят новые настройки и среды без перезапуска

- [ ] `source_folders` у `ConfigOverrides` и `InstalledSimulatorRuntimes`; в `Session` — сборка правил одним местом,
  пересборка в `scan`, признак `touches_rule_sources` в `delta_request` (папка-источник или её прямой ребёнок),
  пересборка и полная сборка рекомендаций в `apply_listings`.
- [ ] Абзац об окружении правил в разделе 4.4 `docs/DOMOVOI.md`.
- [ ] Оставить `crates/app/tests/rules_follow_settings.rs` регрессионным тестом; мутации по Knowledge, по одной на
  прогон.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test rules_follow_settings
```
- Скан, дельта по `.cargo` и дельта по новой папке в нём меняют `cargo.target` и его карточку без перезапуска (exit 0).
```
cargo test -p domovoi-app --test cleanup_session
```
- Открытие сессии по-прежнему берёт `target-dir` из домашнего конфига (exit 0).
```
cargo test -p domovoi-app --test rescan
```
- Полный пересчёт и дельты на образе по-прежнему сходятся с кэшем (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
