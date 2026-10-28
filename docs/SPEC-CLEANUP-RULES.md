<!--
  SPEC-CLEANUP-RULES.md — дельта к docs/SPEC-DOMOVOI.md: новые источники места — журналы программ, отчёты о сбоях,
  вложения из Почты, Корзина Finder и снимки экрана по точному признаку Spotlight. 4 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 28.10.2026, выбран вариант B по рекомендации): логи, отчёты о сбоях и Mail Downloads —
  строки таблицы правил; снимок экрана — только по признаку Spotlight `kMDItemIsScreenCapture`, бейдж в индексе;
  Корзина Finder — своя карточка уровня «Проверьте сами» вместо запрета `DenyReason::Trash`; Корзина iCloud вне дельты.
  На ревью спеки уровень Корзины выбран «Проверьте сами»: из очищенной Корзины файл не вернуть. Отвергнуты: A — снимки
  по имени файла (зависят от языка и переименования, ложные совпадения; против точных признаков направления 1), Корзины
  не видны; C — плюс Корзина iCloud через исключение из защиты iCloud Drive (облачные заглушки ничего не освобождают,
  удаление стирает файл на всех устройствах).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-APP-REMOVAL.md`
  (влита в `main`, `bc94202`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Правила не видят журналы программ (`~/Library/Logs`), отчёты о сбоях (`~/Library/Logs/DiagnosticReports`,
  `/Library/Logs/DiagnosticReports`), копии вложений Почты, Корзину Finder и снимки экрана. `~/.Trash` закрыт
  `DenyReason::Trash`, `Mail Downloads` лежит внутри защищённого `~/Library/Containers/com.apple.*`. Направление взято
  из статического разбора Nektony (MacCleaner Pro, Disk Expert); код и база не переносятся.
- Уже есть и не меняется: приоритет правила — позиция в `rule_table`, `Condition`, `ClaimLedger::settle` с вердиктом
  denylist, движок удаления и лист «К удалению», `PROTECTED_DATA` (кроме одного исключения), `.Trashes` томов,
  `run_query` в `index/src/ffi.rs`, бейджи `Badges` (`u8`, занято четыре бита).
- Найдено разведкой: `Session::open` передаёт `ConfigOverrides::default()` и `Enrichment::default()` — `target-dir`
  и `store-dir` из конфигов в приложении не учитываются, Spotlight не опрашивается. Дельта подключает
  `ConfigOverrides::read` (от него зависит папка снимков) и только запрос снимков; даты последнего открытия из
  Spotlight остаются вне дельты.
- Вне дельты: Корзина iCloud (`~/Library/Mobile Documents/.Trash`, см. Knowledge); остальное в `/Library/Logs` —
  запись только у root, пользователь получил бы одни строки с отказом.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует
  патч; `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/index/src/badges.rs`: `Badges::SCREEN_CAPTURE = 1 << 4`.
- `crates/index/src/ffi.rs`: `SCREEN_CAPTURE_QUERY` = `kMDItemIsScreenCapture == 1`, `spotlight_screen_captures()
  -> io::Result<Vec<PathBuf>>` через `run_query` без атрибутов (только путь).
- `crates/index/src/enrichment.rs`: поле `screen_captures: Vec<PathBuf>`; `gather` заполняет его; публичная
  `query_screen_captures()` для приложения.
- `crates/index/src/index.rs`: `derive` вызывает `apply_screen_captures` — бейдж только узлам `NodeKind::File`,
  найденным по пути; `apply_delta.rs` — то же для узлов, добавленных дельтой. `cache.rs`: `FORMAT_VERSION` на единицу
  больше.
- `crates/rules/src/rule.rs`: `Condition::ScreenCapture`; `also_inside(path)` — шаблон «дети этой папки».
  `rule_set.rs::condition_holds`: `ScreenCapture` — файл с `Badges::SCREEN_CAPTURE`.
- `crates/rules/src/screen_capture_location.rs` (новый): `screen_capture_location(home) -> Option<PathBuf>` —
  `NSUserDefaults(suiteName: "com.apple.screencapture")`, ключ `location`, `~/` раскрывается от `home`.
  `crates/rules/Cargo.toml`: фича `NSUserDefaults` у `objc2-foundation`.
- `crates/rules/src/config_overrides.rs`: поле `screenshot_location`, `with_screenshot_location(Option<PathBuf>)`;
  `read(home)` по-прежнему читает только файлы.
- `crates/rules/src/rule_table.rs` — пять правил после `downloads.installers`, `logs.crash-reports` раньше
  `logs.user`:

  | id | Пути | Уровень | Условие |
  |---|---|---|---|
  | `logs.crash-reports` | `~/Library/Logs/DiagnosticReports/*`, `/Library/Logs/DiagnosticReports/*` | Восстановимо | — |
  | `logs.user` | `~/Library/Logs/*` | Безопасно | — |
  | `mail.downloads` | `~/Library/Containers/com.apple.mail/Data/Library/Mail Downloads/*`, `~/Library/Mail Downloads/*` | Восстановимо | — |
  | `trash.user` | `~/.Trash/*` | Проверьте сами | — |
  | `screenshots` | `~/Desktop/*`, `~/Documents/*`, `~/Downloads/*`, `~/Pictures/*` и `also_inside(location)` | Проверьте сами | `ScreenCapture` |

  Пояснения:
  - `logs.crash-reports` — «Отчёты о сбоях программ и системы. Нужны, чтобы разобраться в повторяющемся сбое;
    после удаления их не вернуть.»
  - `logs.user` — «Журналы программ. Программы начнут новые, их работа не пострадает; пропадёт только история.»
  - `mail.downloads` — «Копии вложений, которые вы открывали из Почты. Письма и вложения в них не пострадают;
    правки, сделанные прямо в открытом вложении, пропадут.»
  - `trash.user` — «Файлы, которые вы уже удалили в Finder. После очистки вернуть их из Корзины будет нельзя.»
  - `screenshots` — «Снимок экрана. Нужен ли он ещё, решаете вы.»
- `crates/rules/src/denylist.rs`: убраны `TRASH` и проверка Корзины; `PROTECTED_EXCEPTIONS` =
  `~/Library/Containers/com.apple.mail/Data/Library/Mail Downloads` — путь в исключении или под ним не проверяется по
  `PROTECTED_DATA`. `deny_reason.rs`: без варианта `Trash`.
- `crates/app/src/strings.rs`: `RULE_TITLES` + «Отчёты о сбоях», «Журналы программ», «Вложения из Почты»,
  «Корзина», «Снимки экрана»; `DENY_TRASH` и его ветка в `deny_reason_text.rs` удалены.
- `crates/app/src/session.rs`: `open` — `ConfigOverrides::read(&paths.home)?` с
  `with_screenshot_location(screen_capture_location(&paths.home))`; перед `Index::build` и перед `apply_delta` —
  `enrichment.screen_captures` из `query_screen_captures()`; ошибка запроса — пустой список и запись в `SystemLog`.
- `docs/DOMOVOI.md` 4.4: пять строк таблицы, абзац о denylist.

# Conventions

- Снимок экрана определяется только признаком Spotlight; имя файла признаком не считается.
- Папки, которыми владеет система (`DiagnosticReports`, `Mail Downloads`, `.Trash`), целиком не забираются: правило
  берёт только их содержимое.
- Denylist абсолютен, кроме одного названного исключения `Mail Downloads`. Исключение снимает только проверку
  `PROTECTED_DATA`; флаги, запущенные бандлы и запись в родителя проверяются как раньше.
- Ошибка запроса Spotlight не останавливает скан: снимков в карточке нет, ошибка в системном журнале.
- Удаляет только движок удаления через «К удалению», без root и без `chflags`.
- Голос: без восклицаний; необратимость названа прямо.

# Knowledge

- Корзина iCloud вне дельты: её файлы часто лишь облачные заглушки и места не занимают, а удаление из неё стирает
  файл на всех устройствах пользователя. Она остаётся под защитой «iCloud Drive».
- Корзина — «Проверьте сами»: уровни 4.4 решаются тем, вернётся ли файл, а из очищенной Корзины он не вернётся
  ниоткуда. Поэтому ассистент очистку Корзины не предлагает (`agent_answer.rs`: действие только не у «Проверьте сами»),
  байты Корзины идут в итог «Проверьте сами»; в «К удалению» карточка отправляется как любая другая.
- `/Library/Logs/DiagnosticReports` принадлежит `root:_analyticsusers` с записью для группы; пользователь-администратор
  в группе, удаление содержимого проходит без root. Без группы строки получат отказ `ParentNotWritable` — это верно.
  Скрытые `.WindowServer-*.ips` и `.contents.panic` лежат там же и уходят вместе с остальными отчётами.
- Отчёты о сбоях — «Восстановимо», а не «Безопасно»: по ним разбирают повторяющиеся сбои (разбор зависания
  WindowServer 28.10.2026 опирался именно на скрытые снимки watchdog).
- Журнал, открытый работающей программой, освобождает место только после того, как она закроет файл.
- Признак снимка: `kMDItemIsScreenCapture == 1`, область — весь компьютер; на машине разработчика 212 результатов за
  ~25 мс. Снимок, который Spotlight ещё не обработал, получит бейдж при следующей дельте его папки или следующем
  скане. Если Spotlight выключен для тома, снимков нет — признак не подменяется именем.
- Снимки на «Рабочем столе» и в «Документах» в iCloud лежат в `~/Library/Mobile Documents` и правилу не видны; так же
  не видны снимки внутри данных программ (истории CleanShot и подобных). Правило смотрит на один уровень в стандартных
  папках и в папке из `com.apple.screencapture`; во вложенные подпапки не спускается — в шаблонах нет якорного `**`.
- Папка снимков читается через `NSUserDefaults`, а не файлом plist: `defaults write` сначала попадает в `cfprefsd`, и
  файл на диске может быть старым.
- Корзина: сама `.Trash` не забирается, только её дети; `.DS_Store` со сведениями «Вернуть» уходит с ними. `.Trashes`
  других томов остаются закрыты.
- `FORMAT_VERSION` повышается: старый кэш без бейджа отвергается, после обновления один полный скан.
- Правила `~/` совпадают с `Users/*`: фикстурам нужен `Users/<имя>/...`, иначе правило уровня «Проверьте сами» в
  тесте не сработает и тест окажется пустым.
- Предел проверки: Spotlight не индексирует тестовый образ, поэтому карточку снимков на главном пути бандла не
  доказать. Её покрывают тесты индекса и правил с подставленным `Enrichment`; на настоящей машине — приёмка человеком
  на границе wire.

# Gameplan

### Phase 1: screen-capture — признак снимка экрана в индексе

- [ ] `Badges::SCREEN_CAPTURE`, `spotlight_screen_captures`, `Enrichment.screen_captures`, `query_screen_captures`.
- [ ] `apply_screen_captures` в `derive`, бейдж узлам дельты в `apply_delta.rs`; `FORMAT_VERSION` на единицу больше.
- [ ] `index/tests/screen_capture.rs` (новый): файл из списка получает бейдж, папка из списка и файл вне списка — нет;
  файл, добавленный дельтой и названный в списке, получает бейдж; бейдж переживает запись и открытие кэша; настоящий
  `query_screen_captures()` отрабатывает без ошибки.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test screen_capture
```
- Бейдж по списку Spotlight при сборке, в дельте и в кэше; настоящий запрос проходит (exit 0).
```
cargo test -p domovoi-index --test cache_roundtrip
```
- Кэш прежней версии отвергается, новый открывается (exit 0).
```
cargo test -p domovoi-index --test last_used
```
- Прежние запросы Spotlight не сломаны (exit 0).
```
cargo clippy -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: cleanup-rules — пять правил, исключение Mail Downloads, Корзина без запрета

<!-- deps: [screen-capture] — потребление: Badges::SCREEN_CAPTURE и Enrichment.screen_captures из фазы 1 в Condition::ScreenCapture. -->

- [ ] `Condition::ScreenCapture`, `also_inside`, `screen_capture_location.rs`, `ConfigOverrides::screenshot_location`.
- [ ] Пять правил в `rule_table` с позициями из Stack.
- [ ] `denylist.rs`: без Корзины, `PROTECTED_EXCEPTIONS`; `deny_reason.rs` без `Trash`.
- [ ] `docs/DOMOVOI.md` 4.4: пять строк таблицы (`trash.user` — «Проверьте сами»); абзац о denylist — исключение
  `Mail Downloads`, Корзина Finder открыта, `.Trashes` и Корзина iCloud закрыты. `doc_parity.rs`: `STARTING_IDS` на
  пять больше.
- [ ] `rules/tests/cleanup_rules.rs` (новый) на настоящем индексе фазы 1 с `Users/<имя>/...`: отчёт в
  `DiagnosticReports` — `logs.crash-reports`, сама папка не забрана, остальное в `~/Library/Logs` — `logs.user`;
  `/Library/Logs/DiagnosticReports/*` забран, `/Library/Logs/other` — нет; вложение в `Mail Downloads` забрано и не
  закрыто denylist, соседний `com.apple.mail/Data/Library/Mail` закрыт; дети `~/.Trash` забраны правилом `trash.user`
  уровня «Проверьте сами» и проходят `settle`, `.Trashes` тома закрыт; файл с бейджем в `~/Desktop` и в папке из
  `screenshot_location` — `screenshots`, тот же файл без бейджа и файл с бейджем в `~/Library` — нет.
- [ ] `rules_table.rs`: фикстура на каждое из пяти правил (бейдж снимка — через `Enrichment.screen_captures`).
  `denylist.rs` (тест): `~/.Trash/old` открыт, `Mail Downloads` открыт, `.Trashes` и `com.apple.mail/.../Mail` закрыты.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test cleanup_rules
```
- Пять правил на настоящем индексе фазы 1, включая снимок по бейджу (exit 0).
```
cargo test -p domovoi-rules --test rules_table
```
- У каждого правила своя фикстура и спокойное пояснение (exit 0).
```
cargo test -p domovoi-rules --test denylist
```
- Корзина Finder и Mail Downloads открыты, остальное под защитой (exit 0).
```
cargo test -p domovoi-rules --test doc_parity
```
- Таблица правил совпадает с документом (exit 0).
```
cargo test -p domovoi-rules --test recommendations_scale
```
- Новый шаблон не замедлил рекомендации на 500 000 файлов (exit 0).
```
cargo clippy -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: cleanup-app — заголовки карточек и Session со Spotlight и конфигами

<!-- deps: [screen-capture, cleanup-rules] — потребление: query_screen_captures (фаза 1); screen_capture_location, ConfigOverrides::with_screenshot_location, идентификаторы правил и DenyReason без Trash (фаза 2). -->

- [ ] `RULE_TITLES` + пять заголовков; `DENY_TRASH` и его ветка удалены.
- [ ] `Session::open` с `ConfigOverrides::read` и папкой снимков; `screen_captures` перед сборкой и дельтой, ошибка —
  пустой список и `SystemLog`.
- [ ] `app/tests/cleanup_session.rs` (новый): `Session::open` на временном `home` учитывает `target-dir` из
  `.cargo/config.toml`; скан временного дерева проходит с настоящим запросом Spotlight. `app/tests/voice.rs`: новые
  заголовки и пояснения без восклицаний.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test cleanup_session
```
- Session читает конфиги и опрашивает настоящий Spotlight фазы 1 (exit 0).
```
cargo test -p domovoi-app --test voice
```
- У каждого правила настоящей таблицы фазы 2 есть заголовок, голос спокойный (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Новые заголовки не обрезаются (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: wire — новые карточки на главном пути бандла

<!-- deps: [screen-capture, cleanup-rules, cleanup-app] — потребление: настоящий индекс, правила и Session в шагах сценария. -->

- [ ] `app/tests/wire_main_path.rs`: на томе — `Users/tester/Library/Logs/Tool/tool.log`,
  `Users/tester/Library/Logs/DiagnosticReports/Tool.ips`, `Library/Logs/DiagnosticReports/Tool.ips`,
  `Users/tester/.Trash/old.zip`, вложение в `Users/tester/Library/Containers/com.apple.mail/Data/Library/Mail
  Downloads/<UUID>/`. Проверки: карточки `logs.user`, `logs.crash-reports`, `mail.downloads`, `trash.user` с этими
  путями; существующий шаг `DeleteRecommendations { level: Safe }` теперь забирает и `logs.user` — ожидания
  `assert_delete_and_clone` сверены; шаг `DeleteRecommendations { level: Recoverable }` удаляет вложение и отчёты о
  сбоях; шаг `DeleteRecommendations { level: CheckYourself }` ставится после проверок остатков удалённого приложения
  и удаляет `old.zip` из Корзины; `.Trash`, обе `DiagnosticReports` и `Mail Downloads` остаются на месте.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла: новые карточки, удаление содержимого при целых системных папках (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-sources
```
- Удаление только в модуле удаления, книга и документ без устаревших формулировок (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
