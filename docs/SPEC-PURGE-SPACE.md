<!--
  SPEC-PURGE-SPACE.md — дельта к docs/SPEC-DOMOVOI.md: Домовой сам освобождает место, которое macOS держит про запас
  (purgeable), через механизм CacheDelete — оценка, запуск с отменой и пределом времени, освобождённое считается по
  росту доступного места; ход работы показывает лист с полосой, как у Корзины. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 29.10.2026, выбран вариант A по рекомендации): закрытые функции CoreServices
  CSDiskSpaceGetRecoveryEstimate / CSDiskSpaceStartRecovery / CSDiskSpaceCancelRecovery через dlsym; нет функций —
  нет пункта меню. Отвергнуты: B — Домовой сам запускает tmutil thinlocalsnapshots (только снапшоты Time Machine, при
  выключенной Time Machine освобождает 0, прямо отменяет «Домовой tmutil не запускает»); C — заставить macOS освободить
  место нехваткой через временный файл (гигабайты записи на SSD, предупреждение системы о заполненном диске, ошибки
  записи у других программ, уборка файла после сбоя). На обзоре спеки принята правка: ход работы показывает лист с
  тремя состояниями по образцу Корзины, а не подсказка строки и пункт меню — долгая работа без видимого хода читается
  как зависание, а второе долгое действие должно выглядеть как первое.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-QUIT-SAVE.md`
  (влита в `main`, `704ab4b`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Строка Purgeable показывает оценку `Important − Available` (`crates/index/src/volumes.rs`,
  `important_available_capacity` в `crates/index/src/ffi.rs`), а освободить это место Домовой не может: единственное
  действие — «Скопировать команду tmutil», и оно касается только снапшотов. Направление взято из статического разбора
  Nektony (Disk Expert, MacCleaner Pro): их слабые места — нет отмены, переоценка опросом каждые 10 с, падение на
  отрицательной разности.
- Уже есть и не меняется: измерение томов и `HiddenSpace`, классы снапшотов, `tmutil_commands` и пункт «Скопировать
  команду tmutil», подсказка строки Purgeable (`hover_tip.rs`), `DeleteJob` / `DeleteRun` как образец задачи с
  таймером, лист Корзины (`TraySheet`, `TrayScreen`, `TrayColumn`, `progress_bar.rs`) как образец листа с ходом
  работы, перемер `DiskOverview` после удаления (`shell_controller.rs`), шаги `hidden_space` и `tmutil_commands` в
  `--e2e`.
- Пересматривается: «удаление снапшотов вне продукта» (`docs/SPEC-DOMOVOI.md`, раздел о томах) — при освобождении
  macOS сама удаляет локальные снапшоты Time Machine, и лист это называет. «Домовой `tmutil` не запускает» остаётся
  в силе: освобождение идёт через CacheDelete, а не через `tmutil`.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч; `.github/workflows/ci.yml`
  версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/index/src/space_recovery.rs` (новый, `pub SpaceRecovery`, `Copy` — набор указателей на функции):
  `load() -> Option<Self>` — `dlsym` трёх функций из уже загруженного CoreServices (`RTLD_DEFAULT`), любая
  отсутствует → `None`; `estimate(self, volume: &Path) -> io::Result<u64>`; `start(self, volume: &Path, bytes: u64)
  -> io::Result<RecoveryHandle>` (хранит идентификатор операции); `cancel(self, handle: &RecoveryHandle) ->
  io::Result<()>`. Экспорт из `lib.rs`.
- `crates/index/tests/space_recovery.rs` (новый).
- `crates/app/src/purge_job.rs` (новый, `pub PurgeJob`): поток — оценка, запуск, опрос доступного места, завершение
  по сигналу механизма либо по остановке роста, предел времени, отмена; `PurgeProgress { estimate, freed }`;
  `PurgeReport { estimate, freed, available_before, available_after, finish: PurgeFinish }`, `PurgeFinish {
  Completed, Stalled, TimedOut, Cancelled }`. Механизм и чтение доступного места — через типаж `SpaceRecovering` и
  замыкание, чтобы тест подставлял свои.
- `crates/app/src/purge_run.rs` (новый, `pub(crate) PurgeRun`): задача плюс таймер `PURGE_TICK`, как `DeleteRun`.
- `crates/app/src/purge_model.rs` (новый, `pub PurgeModel`, без AppKit): состояние листа — `Confirming { estimate,
  time_machine: bool }`, `Releasing(PurgeProgress)`, `Released(PurgeReport)`; `releasing_line()` «Освобождаю: 1,2 ГБ
  из 8,4 ГБ», `releasing_share()` в `0.0..=1.0`, `released_lines()` — «Освобождено: …» и причина остановки.
- `crates/app/src/purge_screen.rs` (новый, `pub PurgeScreen`): экран листа на `TrayColumn` по образцу `TrayScreen` —
  заголовок, по состоянию: подтверждение (оценка, что уходит, кнопки «Отмена» и «Освободить» с
  `setHasDestructiveAction`), работа (строка, полоса `progress_bar`, кнопка «Остановить»), итог (строки итога, кнопка
  «Закрыть»); `Probes` для `truncation.rs`.
- `crates/app/src/purge_sheet.rs` (новый, `pub(crate) PurgeSheet`): окно листа, экран, модель и `Option<PurgeRun>`.
- `crates/app/src/shell_action.rs`: `PurgeSpace`, `ReleasePurge`, `StopPurge`, `ClosePurge`; `shell_actions.rs`:
  селекторы `purgeSpaceForClickedRow:`, `releasePurge:`, `stopPurge:`, `closePurge:`; `level_list.rs`: пункт
  «Освободить…» рядом с `COPY_TMUTIL`; `shell_state.rs`: `recovery: Option<SpaceRecovery>` (один `load()` при
  создании контроллера), `purge: Option<PurgeSheet>`, `offers_purge(node)`; `shell_controller.rs`: открытие листа,
  запуск, тик, остановка, итог, перемер обзора, закрытие.
- `crates/app/src/strings.rs`: `PURGE_SPACE` «Освободить…», `PURGE_TITLE` «Освобождение», `PURGE_RELEASE`
  «Освободить», `PURGE_ESTIMATE` «Можно освободить до», `PURGE_GOES_TIME_MACHINE`, `PURGE_GOES_PLAIN` (что уходит),
  `PURGING` «Освобождаю:», `PURGED` «Освобождено:», `PURGE_STOPPED`, `PURGE_TIMED_OUT`, `PURGE_STALLED`,
  `purge_failed_line(error)`; `STOP`, `CANCEL`, `CLOSE`, `OF` переиспользуются.
- `crates/app/src/scenario_step.rs`, `end_to_end.rs`: шаг `purge_space { timeout_seconds }`.
- Тесты: новые `crates/app/tests/purge_job.rs`, `crates/app/tests/purge_model.rs`; меняются
  `crates/app/tests/truncation.rs`, `crates/app/tests/voice.rs`, `crates/app/tests/wire_main_path.rs`.
- `docs/DOMOVOI.md`: 4-й раздел, абзац о меню строки (сейчас строка 202) — пункт «Освободить…» и лист; 6.5 —
  освобождение через CacheDelete, освобождённое по росту `SPACEAVAIL`. `design/domovoi-design-book.html`: абзац о
  «Скрытом» (строка 628) — действие на строке Purgeable; раздел «Экраны» — лист «Освобождение» рядом с Корзиной,
  три состояния.

# Conventions

- Главный поток не ждёт CacheDelete: вызовы механизма — только в потоке `PurgeJob`; контроллер опрашивает задачу
  таймером, как `DeleteRun`. Оценка на листе подтверждения берётся из уже измеренного `DiskOverview`, не из вызова.
- Ход долгой работы показывает лист с полосой и кнопкой «Остановить», как у Корзины; подсказка строки и контекстное
  меню ход работы не показывают.
- Освобождённое — только измеренное: `max(0, Available_после − Available_до)` по `SPACEAVAIL` через
  `DiskOverview::measure`; оценка механизма в итоге не показывается как освобождённое.
- Арифметика на `u64` — через `saturating_sub`; отрицательная разность показывается как 0; доля полосы не выше 1.
- Нет функций в системе — нет пункта меню и нет ошибок; оценка строки Purgeable показывается как сейчас.
- Тесты не освобождают место на настоящем диске: запуск механизма — только на образе с оценкой 0.
- Действие живёт на строке Purgeable в «Скрытом», не в карточках рекомендаций и не в «К удалению».
- Модель листа (`PurgeModel`) не знает AppKit и проверяется обычным тестом, как `TrayModel` в `tray_deleting.rs`.

# Knowledge

- Purgeable = `Important − Available`: `Important` уже включает место про запас, после освобождения оно почти не
  меняется, растёт `Available`. Поэтому освобождённое считается по росту `SPACEAVAIL`, а не по `Important` (заметка
  разбора Nektony в этом месте неверна).
- Сигнатуры `CSDiskSpace*` не задокументированы. Первая задача фазы `recovery-ffi` — восстановить их по дизассемблеру
  CoreServices из dyld shared cache и закрепить типами в `space_recovery.rs`; тест на Data-томе сверяет оценку с
  purgeable из `Volumes::measure` (оба числа из CacheDelete, допуск — единицы процентов, оценка перечитывается в момент
  сравнения). Если функций нет — гейт красный, прогон останавливается до кода приложения.
- Завершение: если у механизма есть обратный вызов окончания — задача завершается по нему (`Completed`); если нет —
  по остановке роста `Available` на `PURGE_STALL` (5 с) после достижения оценки или после старта (`Stalled`). Опрос
  `SPACEAVAIL` — каждые 0,5 с; предел `PURGE_TIMEOUT` 120 с, по нему посылается отмена и итог считается по
  измеренному (`TimedOut`). Что из этого выбрано — решает восстановленная сигнатура, решение фиксируется в заметке фазы.
- CacheDelete — XPC к `deleted`; запрос может затронуть не только указанный том. В тестах механизм запускается
  только на образе с оценкой 0 байт; освобождение на настоящем диске — приёмка человеком на бандле.
- Том для вызовов — корень Data-тома (`/System/Volumes/Data`), как у `important_available_capacity`, не `/`.
- Лист «Освобождение» повторяет Корзину: открывается `beginSheet` на главном окне, пока открыт — диаграмма под ним
  недоступна, как при удалении. Три состояния в одном окне: подтверждение → работа → итог. Подтверждение называет,
  что уходит: локальные бэкапы Time Machine (если `TimeMachineSettings` объясняет purgeable), загруженные копии
  файлов iCloud, системные кэши; порядок выбирает macOS. Кнопка «Освободить» — с `setHasDestructiveAction`, «Отмена»
  — по Escape, как у `confirm_delete`.
- Работа: строка `PURGING` «Освобождаю: <freed> из <estimate>», доля полосы `freed / estimate`, при оценке 0 — 0,
  не выше 1 при росте сверх оценки. Пока идёт работа, пункт «Освободить…» на строке не предлагается
  (`offers_purge` ложно при открытом листе). Кнопка «Остановить» шлёт отмену, итог считается по измеренному
  (`Cancelled`).
- Итог: «Освобождено: <freed>» и одна строка причины, если работа кончилась не сама: `PURGE_STOPPED` при `Cancelled`,
  `PURGE_TIMED_OUT` при `TimedOut`, `PURGE_STALLED` при `Stalled`; при `Completed` причины нет. Ошибка механизма —
  тоже состояние итога со строкой `purge_failed_line`, приложение не завершается. После итога обзор диска
  перемеряется и экран обновляется, как после удаления; «Закрыть» закрывает лист через `endSheet`.
- `SpaceRecovery::load()` вызывается один раз при создании контроллера и хранится в `ShellState::recovery`; задача
  получает копию (указатели на функции — `Copy`).
- Голос: тексты без восклицаний и слов из списка `voice.rs`; «Освободить…» с многоточием, потому что открывает лист;
  «Освободить» на листе без многоточия, потому что запускает работу.
- Предел проверки: настоящий `CSDiskSpaceStartRecovery` на диске пользователя автотесты не запускают. На бандле
  проверяется: пункт на строке Purgeable при ненулевой оценке, лист подтверждения с оценкой, полоса во время работы,
  рост «Свободно» после освобождения, остановка посреди работы. На машине без purgeable (оценка 0) пункта нет —
  приёмка требует ненулевой оценки.

# Gameplan

### Phase 1: recovery-ffi — закрытые функции CacheDelete через dlsym

- [ ] Восстановить сигнатуры `CSDiskSpaceGetRecoveryEstimate`, `CSDiskSpaceStartRecovery`,
  `CSDiskSpaceCancelRecovery` по дизассемблеру CoreServices; закрепить типами в `space_recovery.rs`.
- [ ] `SpaceRecovery::load` через `dlsym(RTLD_DEFAULT)`; `estimate`, `start`, `cancel`; ошибки механизма — `io::Error`
  с текстом; экспорт из `lib.rs`.
- [ ] `index/tests/space_recovery.rs` (новый): `load()` даёт `Some`; оценка на Data-томе сходится с `purgeable` из
  `Volumes::measure` с допуском; оценка на образе `ImageVolume` равна 0; `start` на образе с 0 байт и `cancel`
  проходят без ошибки.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test space_recovery
```
- Функции найдены, оценка сходится с измерением, запуск и отмена на образе проходят (exit 0).
```
cargo test -p domovoi-index --test volumes
```
- Измерение томов не сломано (exit 0).
```
cargo clippy -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: purge-app — задача освобождения и лист «Освобождение»

<!-- deps: [recovery-ffi] — потребление: SpaceRecovery из фазы 1 в PurgeJob через типаж SpaceRecovering и в ShellState::recovery. -->

- [ ] `purge_job.rs`: `SpaceRecovering` (реализован для `SpaceRecovery`), `PurgeJob::start(recovering, volume,
  available_reader, timeout)`, `progress() -> PurgeProgress`, `cancel()`, `is_finished()`, `finish() ->
  PurgeReport`; завершение по Knowledge.
- [ ] `purge_run.rs`: таймер `PURGE_TICK`, `progress()`, `stop()`, `finish()`, `Drop` гасит таймер.
- [ ] `purge_model.rs`: три состояния, `releasing_line`, `releasing_share`, `released_lines` по Knowledge.
- [ ] `purge_screen.rs`, `purge_sheet.rs`: экран на `TrayColumn` по образцу `TrayScreen` с тремя состояниями и
  `Probes`; лист с окном, экраном, моделью и задачей.
- [ ] Действия и меню: `ShellAction::PurgeSpace` / `ReleasePurge` / `StopPurge` / `ClosePurge`, селекторы в
  `shell_actions.rs`, пункт «Освободить…» в `level_list.rs`; `ShellState::recovery` с одним `load()`,
  `offers_purge(node)` — строка «Скрытое» или Purgeable, оценка больше 0, `recovery` есть, лист не открыт.
- [ ] `shell_controller.rs`: открытие листа подтверждения с оценкой из `DiskOverview`; «Освободить» — `PurgeRun` с
  `Available_до` из `DiskOverview::measure`, лист в состояние работы; тик обновляет строку и полосу; «Остановить» —
  отмена; итог — лист в состояние итога, перемер обзора, `refresh`; «Закрыть» — `endSheet`.
- [ ] `strings.rs`: строки из Stack.
- [ ] `app/tests/purge_job.rs` (новый): на подставном механизме — завершение по обратному вызову, по остановке роста,
  по пределу времени с отправленной отменой, по `cancel`; освобождённое равно росту подставного `Available`;
  отрицательный рост даёт 0; настоящий `SpaceRecovery::load()` оборачивается в `SpaceRecovering` и его `estimate` на
  Data-томе читается без ошибки.
- [ ] `app/tests/purge_model.rs` (новый): строка и доля во время работы, доля 0 при оценке 0, доля не выше 1 при
  росте сверх оценки, строки итога для каждого `PurgeFinish` и для ошибки.
- [ ] `app/tests/truncation.rs`: сборка `PurgeScreen` в трёх состояниях, как у `TrayScreen`.
- [ ] `app/tests/voice.rs`: новые строки без восклицаний и запрещённых слов; `PURGE_SPACE` кончается многоточием,
  `PURGE_RELEASE` — нет.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test purge_job
```
- Задача завершается всеми четырьмя способами, освобождённое считается по росту, настоящий модуль фазы 1 подключён
  (exit 0).
```
cargo test -p domovoi-app --test purge_model
```
- Строки и доля полосы листа во всех состояниях (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Экран листа собирается в трёх состояниях и держит правила обрезки (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Строки освобождения держат голос интерфейса (exit 0).
```
grep -q purgeSpaceForClickedRow: crates/app/src/shell_actions.rs
```
- Пункт меню подключён к действию (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: wire — освобождение на главном пути бандла

<!-- deps: [recovery-ffi, purge-app] — потребление: настоящие SpaceRecovery и PurgeJob в шаге сценария. -->

- [ ] `scenario_step.rs` / `end_to_end.rs`: шаг `purge_space { timeout_seconds }` — `PurgeJob` на томе сценария с
  настоящим `SpaceRecovery`; отчёт: `estimate`, `freed`, `available_before`, `available_after`, `finish`, `error`.
- [ ] `app/tests/wire_main_path.rs`: шаг после `tmutil_commands`; проверки — оценка на образе 0, `freed` 0,
  `available_after >= available_before`, завершение без ошибки, шаг `hidden_space` после него по-прежнему без
  расхождения.
- [ ] `docs/DOMOVOI.md`: абзац о меню строки и 6.5; `design/domovoi-design-book.html`: абзац о «Скрытом» и лист
  «Освобождение» в разделе «Экраны».

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла: оценка, запуск и завершение освобождения на образе через настоящие модули (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-sources
```
- Книга и документ без устаревших формулировок (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
