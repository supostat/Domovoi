<!--
  SPEC-DAISY-SCANNER.md — дельта к docs/SPEC-DOMOVOI.md: четыре приёма сканера из реверса DaisyDisk 4.33.3 на
  публичных API. 5 фаз.

  ГРАНИЦА РЕШЕНИЯ (веер от 27.10.2026, выбран вариант A по рекомендации): сторож файловой системы и доля прогресса
  живут внутри обхода (активность на уровне getattrlistbulk, доля — по каталогам), скрытое место объясняется в index,
  команда tmutil копируется из меню строки «Скрытое». Отвергнуты: B — сторож и доля в приложении по событиям
  прогресса (события теряются при полном буфере и молчат внутри одного большого листинга — ложные обрывы,
  относительной доли нет); C — только сторож и доля (не закрывает задачу). На ревью спеки подтверждено: запасной
  байтовой доли нет, первое событие с долей приходит после листинга корня.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-DAISY-UI.md`
  (влита в `main`, `ce409ec`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Зависший том (сетевой, умирающий диск) держит скан вечно: отмены нет, сообщения нет. Доля прогресса
  считается как `bytes / data_used` (`app/src/scan_job.rs`) — байты приходят рывками, доля зависит от размеров
  файлов, при рескане с кэшем шкала та же. «Скрытое» показывает только purgeable и «Системные данные», не объясняя
  ни домашние папки других пользователей, ни локальные бэкапы Time Machine. Снапшоты перечислены в
  `HiddenSpace.snapshots`, но в интерфейсе их не видно и с ними нечего сделать.
- Уже есть и не меняется: `scan()` с rayon-пулом, `CancelToken`, `Onboarding::wait_until_finished`, протокол
  `ScanProgress::Walking / Walked` (прогресс `try_send` с потерями, `Walked` блокирующим `send`), `ScanStatus`,
  `scan_tick` контроллера, `DockProgress::show(sweep)` и `DiagramState::set_sweep`, `Volumes::measure`,
  `HiddenSpace::new`, `hidden_outline` в `disk_overview.rs`, `fs_snapshot_list` без размеров, `SnapshotClass`,
  `HoverTip` и панель «Свойства», контекстное меню строки уровня (`level_list.rs`).
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/scanner/src/stall_monitor.rs` (новый): `StallMonitor { last_activity: AtomicU64 }` — миллисекунды
  монотонных часов (`Instant` от старта скана); `touch()` после каждого `BulkListing` в `Walk::list` и на каждом
  витке ожидания онбординга; `is_stalled(now, limit) -> bool`; `pub const STALL_LIMIT: Duration = 60 s`. Поток-монитор
  внутри `scan()` опрашивает раз в секунду, при простое ставит `cancel` и флаг `stalled`; по завершении обхода
  монитор останавливается. `scan()` возвращает `ScanError::Unresponsive { path: PathBuf }` — путь последнего
  листинга (`scan_error.rs`). `Onboarding::wait_until_finished` получает `&dyn Fn()` на каждый виток опроса.
- `crates/scanner/src/progress_share.rs` (новый): `ProgressShare { done: AtomicU64 }` с фиксированной точкой
  `SHARE_UNIT = 1 << 40`; `Walk::list` получает долю каталога, делит её поровну между дочерними каталогами
  (`NodeState::Listed`), каталог без дочерних каталогов завершает всю свою долю после листинга; каталог с
  недоступными детьми завершает их доли сам. Абсолютная шкала: `ScanOptions.expected_directories: Option<u64>` —
  при наличии доля = `listed_directories / expected`, clamp 1,0. `ScanProgress::Walking` получает `share: f32`.
- `crates/app/src/session.rs`: `scan()` передаёт `expected_directories` = число каталогов текущего индекса
  (`Index::directory_count()`, новый метод в `crates/index/src/index.rs`, считает `NodeKind::Directory`).
- `crates/app/src/scan_job.rs`: `sweep` берётся из `share` события, деление байт на `expected_bytes` и само поле
  `expected_bytes` удаляются; `ScanJob::start(session, full_first_scan, timer)`.
- `crates/app/src/shell_controller.rs`: `scan_tick` на `Err(SessionError::Scan(ScanError::Unresponsive { path }))`
  показывает `NSAlert` «Файловая система не отвечает» с текстом пути и кнопками «Открыть Дисковую утилиту»
  (`NSWorkspace openApplicationAtURL:` по `URLForApplicationWithBundleIdentifier("com.apple.DiskUtility")`) и
  «Закрыть» (`fail_unresponsive`); сессия остаётся с прежним индексом, как при отмене.
- `crates/index/src/time_machine.rs` (новый): `TimeMachineSettings { auto_backup: Option<bool>, mobile_backups:
  Option<bool> }`, `TimeMachineSettings::read(path) -> Self` через `NSDictionary::dictionaryWithContentsOfFile`
  (objc2-foundation, фича `NSDictionary`), ключи `AutoBackup` и `MobileBackups`, отсутствие файла или ключа — `None`;
  `pub const TIME_MACHINE_PLIST: &str = "/Library/Preferences/com.apple.TimeMachine.plist"`.
- `crates/index/src/volumes.rs`: `HiddenSpace` получает `other_user_homes: u64` (`Index::other_user_homes()` —
  дети корня индекса с именем `Users`, чьи записи `Reach::OtherUser`) и `time_machine: TimeMachineSettings`;
  `overhead` переименовывается в `unaccounted` (смысл тот же: `total − purgeable`); `Volumes::hidden(index,
  settings)`; `HiddenSpace::new` принимает оба новых поля.
- `crates/app/src/disk_overview.rs`: `hidden_outline` — дети «Скрытое»: «Purgeable» (как сейчас), «Не удалось
  найти» вместо «Системные данные» (`UNACCOUNTED`), размер `unaccounted`; `ShellData` получает
  `other_user_homes: u64` и `time_machine: TimeMachineSettings` для подсказок.
- `crates/app/src/hover_tip.rs` и `index_facts.rs`: заметка к «Purgeable» — `PURGEABLE_TIME_MACHINE` («Локальные
  бэкапы Time Machine, macOS удалит их сама») при `auto_backup == Some(true)` или `mobile_backups == Some(true)`,
  иначе `PURGEABLE_PLAIN`; заметка к «Не удалось найти» — `UNACCOUNTED_HINT` плюс «N домашних папок других
  пользователей» при `other_user_homes > 0` (`OTHER_USER_HOMES: PluralForms`); `ShellData::hidden_notice`
  упоминает домашние папки других пользователей отдельно от папок, скрытых macOS.
- `crates/app/src/tmutil_commands.rs` (новый): `tmutil_commands(snapshots: &[Snapshot]) -> String` — по строке
  `tmutil deletelocalsnapshots <YYYY-MM-DD-HHMMSS>` на каждый снапшот класса `TimeMachine` (дата из имени), затем
  `tmutil thinlocalsnapshots / <байты> 4` строкой-подсказкой с комментарием `#`; пустой список → пустая строка.
- `crates/app/src/level_list.rs`, `shell_actions.rs`, `shell_action.rs`, `shell_controller.rs`: пункт меню
  «Скопировать команду tmutil» (`COPY_TMUTIL`, `sel!(copyTmutilForClickedRow:)`, `ShellAction::CopyTmutil`),
  доступен только для строки «Скрытое» или «Purgeable» при непустом списке снапшотов (`allows`); действие пишет
  текст в `NSPasteboard::generalPasteboard()` (`clearContents`, `setString_forType` с `NSPasteboardTypeString`)
  и зовёт `inform(TMUTIL_COPIED)` — «Команда скопирована. Выполните её в Terminal: Домовой tmutil не запускает».
- `crates/app/src/strings.rs`: `UNRESPONSIVE_TITLE`, `UNRESPONSIVE_BODY`, `OPEN_DISK_UTILITY`, `UNACCOUNTED`,
  `UNACCOUNTED_HINT`, `OTHER_USER_HOMES`, `PURGEABLE_TIME_MACHINE`, `PURGEABLE_PLAIN`, `COPY_TMUTIL`,
  `TMUTIL_COPIED`; `SYSTEM_DATA` удаляется.
- `docs/DOMOVOI.md`: 4.1 — сторож и доля прогресса; 4.3 — сектор роста по доле каталогов; 6.5 — объяснение
  purgeable через Time Machine и домашние папки других пользователей; `design/domovoi-design-book.html` — экран 02
  (доля), экран 03 (состав «Скрытого», пункт tmutil), раздел VIII — строки сторожа и tmutil.

# Conventions

- Сторож и доля не меняют протокол событий: `Walking` и `Walked` остаются, `try_send` с потерями сохраняется — доля
  несётся в каждом событии, а не считается по их числу.
- Ожидание онбординга защищённых папок — не простой: каждый виток опроса обновляет активность.
- Домовой не запускает `tmutil` и не читает размер снапшотов: только текст команды в буфер обмена (решение 5
  раздела 6 документа).
- Чтение plist Time Machine — только чтение, отказ (нет файла, нет прав, не словарь) равен `None`, без ошибок в
  интерфейсе.
- Голос раздела 5: «Файловая система не отвечает», «Не удалось найти», без восклицаний; `voice.rs` проверяет.
- Ни одного цвета вне `theme`; комментарии без ссылок на книгу, фазы и спеки; done-when — одна argv-команда.
- Решения 1, 2, 15, 17, 18 документа не меняются.

# Knowledge

- Часы: `Instant` на macOS — `CLOCK_UPTIME_RAW`, он стоит во время сна, поэтому пауза машины не выглядит как простой
  тома и отдельного сброса при пробуждении не нужно; тест сторожа проверяет это подставным временем, а не сном.
- Порог 60 с взят из DaisyDisk (`FileActivityWatchdog`); активность обновляется на каждую пачку `getattrlistbulk`,
  так что каталог из сотен тысяч записей сторожа не будит.
- Монитор ставит `cancel` — обход прекращается тем же путём, что и ручная остановка; отличие только в ошибке
  (`Unresponsive` вместо `Cancelled`), поэтому `Session::scan` при ошибке не трогает прежний индекс, как и сейчас.
- Относительная доля: каталог с одними файлами завершает всю долю после листинга; доля поровну между дочерними
  каталогами даёт монотонный рост без знания размеров; абсолютная шкала при рескане опирается на число каталогов
  прежнего индекса — после больших перемен она может перевалить за 1,0, поэтому clamp.
- Запасной байтовой доли нет: первое `Walking` приходит после листинга корня, до него `sweep` равен нулю в любом
  варианте; два способа считать одну величину противоречат правилу одного способа использования.
- `expected_bytes` и `Volumes.data_used` больше не нужны для прогресса; `Dock` и диаграмма читают тот же `sweep`.
- Доля по каталогам не учитывает размер: диск с одной огромной папкой и сотней пустых растит сектор неравномерно —
  осознанная плата за монотонность и независимость от кэша.
- Размер домашних папок других пользователей публично не измерить (EACCES на листинге), поэтому они входят в
  «Не удалось найти» счётчиком; это честнее, чем оценка.
- `MobileBackups` в plist нового macOS может отсутствовать — тогда объяснение опирается на `AutoBackup`; оба
  отсутствуют — подсказка «Purgeable» без упоминания Time Machine.
- Снапшоты в тестах создавать запрещено (прошлый опыт), поэтому `tmutil_commands` проверяется на синтетическом
  списке `Snapshot`; имя `com.apple.TimeMachine.2026-10-27-101500.local` даёт дату `2026-10-27-101500`.
- Ограничение проверяемости (честно): зависший том, кнопка Дисковой утилиты и буфер обмена живут в живом окне —
  исполняемые критерии доказывают сторож на подставном времени, долю на TempTree, объяснения на фикстурах и
  текст команды; остальное — приёмка человеком после `wire`.

# Gameplan

### Phase 1: scan-watchdog — сторож файловой системы

<!-- deps: [] -->

- [ ] `scanner/src/stall_monitor.rs`, `scan_error.rs::Unresponsive`, обновление активности в `Walk::list` и в
  ожидании онбординга, поток-монитор в `scan()` (`STALL_LIMIT`), `lib.rs` экспорт.
- [ ] `app/src/shell_controller.rs`: `fail_unresponsive` с кнопкой Дисковой утилиты; `strings.rs` строки.
- [ ] `scanner/tests/watchdog.rs` (новый): `StallMonitor` на подставном времени — 59 с тишины не простой, 61 с —
  простой; `touch` сбрасывает; скан TempTree с монитором завершается `Ok` и не трогает лимит; `Unresponsive`
  печатает путь.
- [ ] `app/tests/voice.rs`: строки сторожа без восклицаний.
- [ ] `docs/DOMOVOI.md` 4.1; книга раздел VIII.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-scanner --test watchdog
```
- Сторож на подставном времени и целый скан (exit 0).
```
cargo test -p domovoi-scanner --test fixtures_image
```
- Обход образа с монитором не изменил поведения (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Строки сторожа по голосу интерфейса (exit 0).
```
cargo clippy -p domovoi-scanner -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

> Отменено после приёмки: доля каталогов не связана с байтами, а углы диаграммы байтовые, поэтому сектор шёл вразнобой со счётчиком в центре. Сектор и полоса в Dock снова считаются по байтам к занятому месту тома; `ProgressShare`, `expected_directories` и `Index::directory_count` удалены.

### Phase 2: scan-share — доля прогресса по каталогам

<!-- deps: [scan-watchdog] — order-only: обе фазы правят Walk::list, последовательность исключает конфликт. -->

- [ ] `scanner/src/progress_share.rs`, `ScanProgress::Walking.share`, `ScanOptions.expected_directories`,
  раздача долей в `Walk::list`.
- [ ] `index/src/index.rs::directory_count`; `app/src/session.rs` передаёт ожидаемое число каталогов;
  `app/src/scan_job.rs` берёт `share`, `expected_bytes` удаляется; вызовы `ScanJob::start` в контроллере и
  `end_to_end.rs`.
- [ ] `scanner/tests/scan_share.rs` (новый): доля на TempTree растёт монотонно и в `Walked` равна 1,0; дерево из
  одних файлов даёт 1,0 первым же событием; `expected_directories` вдвое больше реального → доля в конце 0,5,
  вдвое меньше → clamp 1,0.
- [ ] `docs/DOMOVOI.md` 4.3; книга экран 02.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-scanner --test scan_share
```
- Относительная и абсолютная доля (exit 0).
```
cargo test -p domovoi-app --test rescan
```
- Рескан с ожидаемым числом каталогов проходит прежний путь (exit 0).
```
cargo test -p domovoi-app --test screen_update
```
- Экран скана и «Диск» с новым `sweep` (exit 0).
```
cargo clippy -p domovoi-scanner -p domovoi-index -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: hidden-space — домашние папки других пользователей и Time Machine

<!-- deps: [] -->

- [ ] `index/src/time_machine.rs`, `Index::other_user_homes`, `HiddenSpace { other_user_homes, time_machine,
  unaccounted }`, `Volumes::hidden(index, settings)`; фича `NSDictionary` в `index/Cargo.toml`.
- [ ] `app/src/disk_overview.rs` («Не удалось найти»), `shell_data.rs` (поля и `hidden_notice`), `hover_tip.rs`,
  `index_facts.rs`, `strings.rs`; фикстура `shell_state.json`, если ключи добавляются.
- [ ] `index/tests/volumes.rs`: `HiddenSpace::new` с двумя домашними папками других пользователей и настройками
  Time Machine — `unaccounted` равен прежнему `overhead`, счётчик виден; `index/tests/time_machine.rs` (новый):
  plist-фикстуры `tests/fixtures/time-machine-on.plist` (AutoBackup, MobileBackups) и `-off.plist`, отсутствующий
  файл → `None`; `app/tests/voice.rs`: новые подсказки; `app/tests/snapshot_shell.rs`: экран 03 держит размеры.
- [ ] `docs/DOMOVOI.md` 6.5; книга экран 03.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test volumes
```
- Скрытое место с домашними папками других пользователей (exit 0).
```
cargo test -p domovoi-index --test time_machine
```
- Чтение настроек Time Machine из plist (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Подсказки без восклицаний (exit 0).
```
cargo test -p domovoi-app --test snapshot_shell
```
- Снимки с новым составом «Скрытого» (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: tmutil-clipboard — команда tmutil в буфер обмена

<!-- deps: [hidden-space] — потребление: пункт меню доступен по HiddenSpace.snapshots из ShellData фазы 3. -->

- [ ] `app/src/tmutil_commands.rs`; `ShellAction::CopyTmutil`, селектор, пункт меню строки уровня, `allows` по
  строке «Скрытое»/«Purgeable» и непустым снапшотам; запись в `NSPasteboard` (фича `NSPasteboard` уже есть);
  `inform(TMUTIL_COPIED)`.
- [ ] `app/tests/tmutil_commands.rs` (новый): два снапшота Time Machine и один `os.update` → две строки
  `deletelocalsnapshots` с датами из имён и строка `thinlocalsnapshots`; пустой список → пустая строка; нет
  восклицаний.
- [ ] `docs/DOMOVOI.md` 4.8; книга экран 03 и раздел VIII.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test tmutil_commands
```
- Текст команды из списка снапшотов (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Строка подтверждения по голосу интерфейса (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 5: wire — сквозной путь через бандл

<!-- deps: [scan-watchdog, scan-share, hidden-space, tmutil-clipboard] — потребление: сценарий гонит настоящий scan() с монитором и долей, Volumes::hidden с настройками Time Machine и tmutil_commands. -->

- [ ] `app/src/scenario_step.rs`, `end_to_end.rs`: шаг `scan` отчитывается `share_events` (число событий с долей,
  максимум доли, доля в `Walked`), `rescan` — `expected_directories` и доля в конце; новый шаг `HiddenSpace`
  (отчёт `unaccounted`, `other_user_homes`, `time_machine`, `snapshots`); новый шаг `TmutilCommands` (отчёт числа
  строк).
- [ ] `app/tests/wire_main_path.rs`: доля первого скана доходит до 1,0 и не убывает, рескан несёт
  `expected_directories > 0`, скрытое место отчитывается без расхождения, число строк tmutil равно числу
  снапшотов Time Machine тома (на образе обычно 0 — утверждение по равенству, не по положительности).
- [ ] Граница приёмки человеком: зависший сетевой том и кнопка Дисковой утилиты, буфер обмена с командой, объяснение
  purgeable на машине с включённым Time Machine.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь через бандл с настоящими модулями всех четырёх фаз (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-tokens
```
- Цвета только в теме (exit 0).
```
cargo clippy -p domovoi-scanner -p domovoi-index -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
