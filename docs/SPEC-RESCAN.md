<!--
  SPEC-RESCAN.md — дельта к docs/SPEC-DOMOVOI.md: ручной полный пересчёт диска — кнопка в блоке диска
  сайдбара и пункт меню ⌘R. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 19.10.2026, выбран вариант A по рекомендации; на ревью спеки: решение гейта
  вынесено в чистый ShellGate; кнопка — в блоке диска сайдбара: по снимку 03 шапка «Диска» не вмещает
  текстовую кнопку, а значок ↻ в шапке требует сворачивать крошки по ширине — это давний дефект шапки,
  отдельный план): ShellAction::Rescan за гейтом Session и закрытым листом Корзины Домового, пересчёт —
  существующий begin_scan, «Остановить» возвращает прежний индекс (так уже работает Session::scan), без
  подтверждения. Отвергнуты: B — отменять идущее и ждать Session (молча теряет поиск дубликатов и ответ, нужен
  диалог, ApplyJob не отменить, флаг в трёх точках возврата Session, гонки); C — освобождать индекс на время
  пересчёта (пик памяти вдвое ниже, но «Остановить» перечитывает 792 МБ кэша, а при обрыве истории FSEvents
  сразу начинается полный скан).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `SPEC-AGENT-PATHS` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема: пересчитать диск заново можно только удалив `~/Library/Caches/com.domovoi.app/scan.cache`
  вручную; полный скан идёт лишь без кэша или со сброшенным кэшем (`open_session`) и по RootChanged / Unmount /
  ошибке дельты (`events_tick`). После отмены первого скана просканировать снова можно только перезапуском.
- Уже есть и не меняется: экран 02 с «Остановить», `ScanJob`, `Session::scan` (кэш пишется в потоке скана),
  таблица `ShellAction::needs_session` и проверка `allows` в `ShellActions::dispatch`; поиск дубликатов,
  вопрос и тяжёлая пачка забирают Session в поток; шапка экрана «Диск» и крошки.
- Вне дельты: крошки шапки по ширине не ограничены — на глубоком пути с «в облаке» они заходят под
  переключатель; давний дефект, отдельный план.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам
  инкрементирует патч; `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/app/src/shell_action.rs`: `ShellAction::Rescan`, входит в `needs_session`.
- `crates/app/src/shell_gate.rs` (новый, экспорт из `lib.rs`): `ShellGate { session_open, tray_open,
  searching }` — три факта состояния; `allows(action)`: `ToggleDuplicateSearch` — идёт поиск или Session
  открыта (перенос ветки из контроллера), `Rescan` — Session открыта и лист Корзины закрыт, остальные —
  `!needs_session() || session_open`.
- `crates/app/src/shell_state.rs`: `ShellState::gate()` собирает `ShellGate` из `session`, `tray`, `search`.
- `crates/app/src/shell_controller.rs`: `allows` — ветки путей узлов (`ShowInFinder`,
  `ShowClickedRowInFinder`) на месте, остальное `state.gate().allows(action)`; `perform(Rescan)` — новый
  `DiskOverview::measure`, Session забирается, `begin_scan(session, false)`; `rebuild_current_views` перед
  сборкой экрана ставит `data.rescan_allowed = state.gate().allows(ShellAction::Rescan)` (рядом с
  `sync_assistant`).
- `crates/app/src/shell_data.rs`: поле `rescan_allowed: bool` (`false` в `measured`).
- `crates/app/src/strings.rs`: `RESCAN` = «Пересканировать», `RESCAN_DISK` = «Пересканировать диск».
- `crates/app/src/shell_actions.rs`: селектор `rescanDisk:` → `dispatch(ShellAction::Rescan)`.
- `crates/app/src/sidebar_panel.rs`: поле `rescan` — `ShellButton` в блоке диска над полосой «свободно»,
  `setEnabled(rescan_enabled)`; `DISK_BLOCK` растёт на высоту кнопки и отступ; экспорт из `lib.rs` для теста.
- `crates/app/src/window_frame.rs`: передаёт `data.rescan_allowed` в `SidebarPanel::build`.
- `crates/app/src/shell_fixture.rs`: в снимках флаг как в оболочке — на экране 02 кнопка серая, на
  остальных активна.
- `crates/app/src/main_menu.rs`: пункт `RESCAN_DISK`, клавиша `r` (⌘R), действие `rescanDisk:`, без target;
  `main_menu` экспортируется из `lib.rs` для теста.
- `crates/app/src/app_delegate.rs`: `rescanDisk:` → `ShellActions::dispatch(ShellAction::Rescan)` контроллера;
  `validateMenuItem` для `rescanDisk:` → `allows(ShellAction::Rescan)`.
- `crates/app/src/end_to_end.rs`, `scenario_step.rs`: шаг `rescan { path }` через настоящий `ScanJob`.
- Тесты: новые `crates/app/tests/rescan.rs`, `crates/app/tests/sidebar_rescan.rs` (`harness = false`,
  `[[test]]` в `crates/app/Cargo.toml`); меняются `session_gate.rs`, `wire_main_path.rs`.

# Conventions

- Решение гейта принимает `ShellGate`; контроллер только собирает факты (`ShellState::gate`) и не
  ветвится по действиям сверх путей узлов.
- Один вход: кнопка и пункт меню идут через `ShellActions::dispatch` → `allows`; `perform` в обход
  `dispatch` не вызывается. Кнопка серая ровно тогда, когда `allows(Rescan)` ложно.
- Новых задач и состояний нет: пересчёт — `begin_scan`, остановка — существующий `StopScan`.
- Тексты — только в `strings.rs` и проходят `voice`.

# Knowledge

- «Остановить» возвращает прежний индекс уже сейчас: `Session::scan` выходит на `scan(...)?` до `install`,
  поэтому `index` и `last_event_id` не меняются; `scan_tick` на `Cancelled` вызывает `install`, и FSEvents
  догоняют пропущенное с прежнего `last_event_id`. Частичного индекса нет: на отмене сканер не отдаёт отчёт.
  Подтверждение перед пересчётом не нужно — остановка его полностью откатывает.
- После отменённого первого скана Session есть, а индекса нет: `Rescan` разрешён и запускает полный скан
  без перезапуска приложения.
- Гейт: поиск дубликатов, ответ ассистента и тяжёлая пачка держат Session — кнопка и пункт меню серые.
  Лист Корзины Домового Session не держит, но `measure_tick → refresh_tray → measured_tray` берёт её через
  `expect` — пересчёт при открытом листе уронил бы приложение. Лист модален для окна, поэтому до него
  дотягивается только ⌘R; `ShellGate::allows(Rescan)` требует `!tray_open`.
- `ShellGate` — чистый тип без AppKit: тест строит его литералом и проверяет решение целиком. Связка
  `ShellState::gate()` — три чтения полей; `TraySheet` держит `NSWindow`, поэтому `ShellState` с открытым
  листом в тесте не собирается.
- Место кнопки: по снимку 03 (окно 1100 pt) правая группа шапки «Диска» начинается с x ≈ 587, крошкам
  остаётся ≈ 316 pt; текстовая кнопка с отступом — ≈ 156 pt, свёрнутый глубокий путь ≈ 270 pt — не
  помещается. Блок диска сайдбара (ширина 230, поле 202) рядом с «свободно X из Y» свободен и виден с любого
  экрана. Кнопка — стиль «Остановить» экрана 02 (`fill: None`, рамка `palette.line()`, текст `tokens.text`,
  `interface_font(13, 500)`), над полосой, от левого края `INSET`; проба снимка сайдбара (`[8, 8]`) левее
  `INSET` и не задевается.
- Состояние кнопки: сайдбар общий для всех экранов, поэтому кнопка серая по гейту, а не молча бездействует.
  Флаг ставится при каждой сборке экрана (`rebuild_current_views`); старт и конец поиска, вопроса и скана
  пересобирают экран. Тяжёлая пачка экран не пересобирает — на её секунды кнопка остаётся активной, а нажатие
  отклоняет `dispatch` (как «Корзину Домового» сейчас).
- Меню строится в `interactive.rs` до контроллера; пункты без target идут по цепочке ответчиков до
  `AppDelegate` (как `checkForUpdates:`), `ShellActions` в цепочке нет — делегат передаёт действие в
  `dispatch`, где проверяется `allows` (bugfix «dispatch вызывал perform без allows»).
- `DiskOverview` перемеряется перед пересчётом: «свободно X» и ожидаемый объём для сектора скана иначе
  остаются со времени запуска.
- На время пересчёта окно на экране 02, как при первом скане; после `install` — экран «Диск».
- Память: пока строится новый индекс, прежний остаётся в Session — пик около двух индексов плюс отчёт
  сканера, как сейчас при RootChanged. Освобождение индекса на время пересчёта (вариант C) — отдельной
  дельтой, если пик станет проблемой.
- e2e: `ScanJob::start` принимает таймер оболочки; в `EndToEnd` цикла событий нет, поэтому шаг `rescan`
  отдаёт неплановый `NSTimer` (`timerWithTimeInterval…`, `finish` его гасит) и сам опрашивает
  `absorb_progress` / `is_finished` — канал прогресса читается параллельно обходу, иначе обход встанет на
  последнем `send`.
- Граница гейтов: тесты доказывают контракт `Session` (остановка не трогает индекс, пересчёт находит новый
  файл и переписывает кэш), решение гейта целиком (`ShellGate`: Session, лист Корзины, «Стоп» поиска), кнопку
  в сайдбаре (заголовок, селектор, серость по флагу, место в блоке диска), пункт меню, полный пересчёт через
  настоящий `ScanJob` в бандле. Не доказывает гейт: связки `ShellState::gate()` и `data.rescan_allowed` и
  маршрут `perform` — `ShellController` в тестах не собирается (`launch` открывает настоящий корень и проверяет
  FDA). Приёмка человеком: кнопка и ⌘R серые при открытом листе Корзины, во время поиска дубликатов, ответа и
  скана; «Пересканировать» → экран 02 → «Остановить» → прежняя диаграмма; полный пересчёт → новые цифры и
  «свободно».

# Gameplan

### Phase 1: rescan-action — действие, гейт, контракт остановки

<!-- deps: [] -->

- [ ] `ShellAction::Rescan`, `ShellGate`, `ShellState::gate()`, `allows` и `perform(Rescan)` в контроллере по
  Stack.
- [ ] Тест `crates/app/tests/rescan.rs` на APFS-образе (`ImageVolume`, как в `wire_agent_paths`): скан →
  новый файл → скан с заранее отменённым `CancelToken` даёт `SessionError::Scan(ScanError::Cancelled)`,
  индекс без нового файла, `last_event_id` и число узлов прежние; полный пересчёт находит файл,
  `last_event_id` растёт, новая `Session` через `open_cached` открывает кэш (`CacheOpening::Opened`) с этим файлом.
- [ ] `crates/app/tests/session_gate.rs`: `Rescan` в таблице действий, которым нужна Session; `ShellGate`:
  `Rescan` разрешён при открытой Session и закрытом листе, отклонён без Session и при открытом листе;
  `ToggleDuplicateSearch` разрешён во время поиска без Session («Стоп») и отклонён без Session и поиска;
  действия из таблицы `needs_session` отклонены без Session; лист Корзины не закрывает действия кроме `Rescan`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test rescan
```
- Остановка оставляет прежний индекс, пересчёт находит новое и пишет кэш (exit 0).
```
cargo test -p domovoi-app --test session_gate
```
- Решение гейта: пересчёт ждёт Session и закрытый лист Корзины (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: rescan-controls — кнопка в блоке диска сайдбара и ⌘R

<!-- deps: [rescan-action] — потребление: кнопка и пункт меню отправляют настоящий ShellAction::Rescan, серость — из ShellGate. -->

- [ ] Строки, селектор `rescanDisk:`, `ShellData::rescan_allowed` и его установка в `rebuild_current_views`,
  кнопка в `SidebarPanel`, передача флага в `WindowFrame`, флаг в `ShellFixture`, пункт меню, `AppDelegate`
  по Stack и Knowledge.
- [ ] Тест `crates/app/tests/sidebar_rescan.rs` (`harness = false`):
  - `SidebarPanel` на данных `tests/fixtures/shell_state.json`: у `rescan` заголовок `RESCAN`, действие
    `rescanDisk:`, target — `ShellActions`;
  - `isEnabled` совпадает с переданным флагом (оба значения);
  - кадр `rescan` внутри `INSET … ширина − INSET`, ниже прокрутки списка и не пересекает полосу и «свободно»;
  - в меню приложения `main_menu` есть пункт `RESCAN_DISK` с клавишей `r` и действием `rescanDisk:`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test sidebar_rescan
```
- Кнопка в блоке диска и пункт меню на месте, шлют `rescanDisk:`, серость по флагу (exit 0).
```
cargo test -p domovoi-app --test session_gate
```
- REAL-DEP: настоящие ShellAction::Rescan и ShellGate (exit 0).
```
cargo test -p domovoi-app --test snapshot_shell
```
- Все экраны собираются с новым блоком диска (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые строки проходят правила голоса (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: wire — пересчёт на главном пути бандла

<!-- deps: [rescan-action, rescan-controls] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `ScenarioStep::Rescan { path }` («rescan»): `EndToEnd` отдаёт Session настоящему `ScanJob` (таймер по
  Knowledge), читает прогресс до конца, забирает Session обратно; отчёт
  `{ "step": "rescan", "found", "files", "last_event_id" }`.
- [ ] `crates/app/tests/wire_main_path.rs`: после `reopen_cache` — `create_file` без `await_change` и сразу
  `rescan` с его путём: `found == true`, `last_event_id` больше, чем у `reopen_cache`; следующие шаги сценария
  (включая снимки) проходят как прежде.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` выполняется задачей до гейта.
- [ ] `docs/SPEC-DOMOVOI.md`: сайдбар — «Пересканировать» в блоке диска и ⌘R; Knowledge — «Остановить»
  возвращает прежний индекс, пересчёт за гейтом Session и закрытым листом Корзины.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Пересчёт через настоящий ScanJob находит файл без событий FSEvents; снимки из бандла с новым сайдбаром (exit 0).
```
cargo xtask check-sources
```
- Согласованность документов (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
