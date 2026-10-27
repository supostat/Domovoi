<!--
  SPEC-DAISY-UI.md — дельта к docs/SPEC-DOMOVOI.md: пять приёмов интерфейса из реверса DaisyDisk 4.33.3 на
  публичных API. 6 фаз.

  ГРАНИЦА РЕШЕНИЯ (веер от 27.10.2026, выбран вариант A по рекомендации): весь набор на публичных API, «Свойства» —
  собственная панель из данных наведения и индекса, без Apple Events. Отвергнуты: B — «Свойства» через окно Finder
  по Apple Events (entitlement automation.apple-events, системный запрос «управлять Finder» ради одного пункта меню,
  не проверяется вне подписанного бандла); C — ядро без Quick Look, уведомлений и «Свойств» (быстрее, но не закрывает
  задачу, а Quick Look — самый заметный приём DaisyDisk). На ревью спеки Quick Look ограничен списком уровня и
  дубликатами: Корзина Домового остаётся стопкой подписей без выделения, её строки доступны для предпросмотра из
  списка уровня.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/SPEC-PERMANENT-DELETE.md` (влита в `main`, `2c3bef6`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Реверс DaisyDisk 4.33.3 (SmallerItemsGroupItem по minVisualizableRelativeSize, QLPreviewPanel в списках,
  NSDockTile с прогрессом и уведомление по окончании, перетаскивание иконки в список Full Disk Access, пункты
  контекстного меню Terminal и Info, подписи «Music ▸ Music», экспорт CSV) против Домового сегодня:
  `render/src/sunburst_layout.rs:98` отбрасывает дуги уже `VISIBLE_SPAN` без следа, кольцо не сходится с родителем;
  предпросмотра нет; Dock и уведомления не используются; на экране доступа только текст шагов и кнопки;
  меню списка уровня — один пункт «Показать в Finder» (`level_list.rs:37`); экспорта нет.
- Уже есть и не меняется: `DiagramTree`, `DiagramState` (hover, click, keyboard focus), `accessible_segments`,
  treemap целиком; `LevelList` на `NSTableView` с `LevelListSource` и `level_row_cell.rs`; `HoverTip` и `HoverPanel`;
  `ShellActions` и `ShellAction`; `AccessState` и проба `FullDiskAccess::probe`; кнопка «Перезапустить» в состоянии
  `GrantedAfterLaunch` (`access_screen.rs:252`, `shell_controller.rs:1036`) — пункт 4 задачи о кнопке уже выполнен,
  новое только перетаскивание; `ScanStatus.sweep`; `main_menu.rs`; снимки `--snapshot` и их фикстура
  `tests/fixtures/shell_state.json`; экран Корзины Домового (`tray_screen.rs`, `tray_column.rs`, `TrayRow`).
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/render/src/segment_kind.rs` (новый): `SegmentKind { Node, SmallGroup { parent: usize, members: Vec<usize> } }`.
  `ArcSegment` получает `kind: SegmentKind`; `node` остаётся узлом для `Node` и родителем для группы.
- `crates/render/src/sunburst_layout.rs`: `walk` собирает дочерние дуги уже `VISIBLE_SPAN` в одну группу на каталог в
  конце его сектора (угол — сумма остатков), кольцо сходится с родителем; `compute(tree, focus, size, sweep,
  expanded: &SmallGroups)`, где `SmallGroups { always: bool, opened: BTreeSet<usize> }` (`small_groups.rs`, новый):
  раскрытая группа кладёт членов отдельными дугами без порога. `hit_test -> Option<Hit>`, `Hit { Node(usize),
  Group(usize) }` (`hit.rs`, новый). Константа `VISIBLE_SPAN` становится `pub` как `SMALL_GROUP_SPAN`.
- `crates/render/src/diagram_state.rs`: `small_groups: SmallGroups`; `click` по `Hit::Group` открывает группу и
  перерисовывает, по `Hit::Node` — как сейчас; `set_small_groups_always(bool)`; клавиатурный фокус обходит группу
  как элемент и раскрывает её по Enter.
- `crates/render/src/accessibility_model.rs`: группа — `AccessibleSegment` с подписью
  `DiagramStrings::small_group(count, size)` («мелкие объекты: 37, 1,2 ГБ»); `diagram_strings.rs` получает строку.
- `crates/render/src/diagram_painter.rs`: дуга группы рисуется цветом категории `Category::Other` с приглушением
  (токен темы, без литералов цвета).
- `crates/app/src/diagram_preference.rs`: ключ `ExpandSmallGroups` в `NSUserDefaults`; `main_menu.rs` — меню «Вид»
  с пунктом `ALWAYS_EXPAND_SMALL` (галочка, `sel!(toggleSmallGroups:)`); `ShellAction::ToggleSmallGroups`.
- `crates/app/src/nested_name.rs` (новый): `display_name(tree, node) -> String` — «Music ▸ Music», когда имя узла
  совпадает с именем родителя (разделитель ` ▸ ` в `strings.rs`). Используется в `shell_data.rs::level_rows`
  (`LevelRow.name`), `HoverTip.name`, `center_text` через `ShellData`. Крошки не меняются (там родитель виден).
- `crates/app/src/node_facts.rs` (новый): `NodeFacts { tip: HoverTip, created: Option<String>, modified: Option<String>,
  entries: Option<u64> }` из `IndexNode.created`, `modified` и числа записей; `properties_panel.rs` (новый) —
  `NSPopover` над строкой списка или сегментом, поля как у `HoverPanel` плюс даты и кнопка «Показать в Finder».
- `crates/app/src/level_list.rs`: меню получает «Открыть в Terminal» (`sel!(openClickedRowInTerminal:)`) и
  «Свойства» (`sel!(showClickedRowProperties:)`); Terminal — `NSWorkspace openURLs:withApplicationAtURL:` с URL
  Terminal из `URLForApplicationWithBundleIdentifier:` (`com.apple.Terminal`), каталог узла или родитель файла.
  Диаграмма: те же два пункта в меню сегмента (`shell_controller.rs`, где строится меню по `ShowInFinder`).
- `crates/app/src/csv_export.rs` (новый): `csv_lines(index, root: NodeId) -> String` — заголовок
  `path,bytes,category,modified`, обход индекса в глубину, экранирование по RFC 4180; `main_menu.rs` — «Файл ▸
  Экспортировать список…» (`sel!(exportCsv:)`), `NSSavePanel` с `nameFieldStringValue` «Домовой — <том>.csv»,
  запись через `std::fs::write`, отказ — `fail`.
- Quick Look: workspace-зависимость `objc2-quick-look-ui = "=0.3.2"` (фичи `QLPreviewPanel`, `QLPreviewItem`);
  `crates/app/src/preview_source.rs` (новый) — объект `QLPreviewPanelDataSource` + `QLPreviewPanelDelegate` над
  `PreviewTargets { urls: Vec<NSURL> }`; `preview_targets.rs` (новый) — чистая выборка путей из выделения:
  `from_level(rows, selected, index)`, `from_duplicates(groups, selected)`. `ShellRootView` отвечает
  `acceptsPreviewPanelControl` и `beginPreviewPanelControl`; пробел в двух списках — `keyDown` подкласса таблицы
  списка уровня (`preview_table.rs`, новый) и подкласса outline в дубликатах → `ShellAction::TogglePreview`.
  Корзина Домового предпросмотра не получает: её строки — стопка подписей без выделения, те же элементы доступны
  из списка уровня.
- `crates/app/src/duplicates_source.rs`: `shouldSelectItem` → `true` для строк участников, `false` для заголовков групп.
- Dock и уведомление: `crates/app/src/dock_progress.rs` (новый) — `DockProgressBar` (`NSView`, рисует полосу по
  `sweep` цветами токенов темы) в `NSDockTile.contentView`; `DockProgress::show(sweep)`, `clear()` после скана;
  `scan_tick` вызывает `show`, завершение — `clear`. `scan_notice.rs` (новый): `scan_finished_line(elapsed) ->
  String` «Скан завершён за 2 мин 10 с» (единицы `strings.rs`, десятичные правила решения 17 не затрагиваются).
  Workspace-зависимость `objc2-user-notifications = "=0.3.2"`; `notification_center.rs` (новый): `ScanNotifier` —
  при первом старте скана из бандла (`NSBundle.bundleIdentifier` есть) запрашивает `requestAuthorizationWithOptions`
  (alert), по завершении скана, если `NSApplication.isActive` ложно, ставит `UNNotificationRequest` с заголовком
  `APP_NAME` и телом строки; вне бандла — ничего (снимки и тесты запускают голый бинарник).
- `crates/app/src/app_icon_drag.rs` (новый): `AppIconView` — подкласс `NSImageView` с `NSDraggingSource`:
  `mouseDown` начинает `beginDraggingSessionWithItems:event:source:` с `NSPasteboardItem` (`public.file-url` бандла),
  `sourceOperationMaskForDraggingContext` → `Copy`; виден только из бандла. `access_screen.rs`: иконка слева от шага
  3, текст `ACCESS_STEPS[2]` — «перетащите эту иконку в список Full Disk Access или нажмите +»; `ACCESS_DRAG_HINT`.
- `docs/DOMOVOI.md`: 4.3 — абзац о группе «мелкие объекты» и hit-test группы; 4.8 — Quick Look (список уровня и
  дубликаты), Dock, уведомление, экспорт CSV, перетаскивание на экране 1, контекстное меню узла.
  `design/domovoi-design-book.html`: экран 01 — иконка и текст шага; экран 03 — пункт меню и группа в кольце;
  экран 06 — предпросмотр; раздел VIII — строка «Скан завершён за …».
- `crates/app/src/strings.rs`: `SMALL_GROUP`, `ALWAYS_EXPAND_SMALL`, `OPEN_IN_TERMINAL`, `PROPERTIES`, `CREATED`,
  `MODIFIED`, `ENTRIES`, `EXPORT_CSV`, `CSV_NAME`, `SCAN_FINISHED_IN`, `MINUTES_SHORT`, `SECONDS_SHORT`,
  `ACCESS_DRAG_HINT`, `NESTED_NAME_SEPARATOR`; все без восклицаний (тест `voice.rs`).

# Conventions

- Только публичные API: `QLPreviewPanel`, `UNUserNotificationCenter`, `NSDockTile`, `NSDraggingSource`,
  `NSSavePanel`, `NSWorkspace`. Apple Events к Finder не используются.
- Treemap не меняется: `treemap_layout.rs`, `squarify.rs`, `tile_*` — без правок. Экран Корзины Домового не меняется.
- Ни один цвет вне `theme` (`cargo xtask check-tokens`); комментарии без ссылок на книгу, фазы и спеки
  (`cargo xtask check-comments`).
- Новые крейты objc2 — ровно `=0.3.2`, как остальные в `Cargo.toml`; фичи подбираются по компилятору (прошлый опыт:
  методы спрятаны за фичами `NSOperation`, `block2`, `NSAccessibility`).
- Всё, что зависит от бандла (уведомления, Dock-плитка, иконка для перетаскивания), молча отсутствует в голом
  бинарнике: снимки и тесты не падают.
- Группа идентифицируется родителем, не отдельным номером узла: `MainScreen::update` переписывает номера узлов
  (`screen_update.rs`), синтетический номер за пределами `DiagramTree` недопустим.
- Голос раздела 5: без восклицаний; «мелкие объекты», «Скан завершён за …» — спокойно и по делу.
- Done-when — одна argv-команда с конкретной целью (контракт CLAUDE.md).

# Knowledge

- Сходимость кольца: сумма углов кольца 0 равна `TAU` только если остаток уходит в группу; тест
  `every_ring_of_the_sunburst_closes_the_full_circle…` сейчас утверждает «slivers dropped» и переписывается:
  каждая дуга-узел шире `SMALL_GROUP_SPAN`, группа одна на каталог, её угол — ровно остаток.
- Раскрытая группа кладёт членов без порога, поэтому очень тонкие дуги в раскрытом состоянии допустимы; «всегда
  раскрывать» — осознанный выбор пользователя.
- `QLPreviewPanel` ищет контроллер по цепочке респондеров от первого респондера: таблица должна быть первым
  респондером, корневой вид окна отвечает `acceptsPreviewPanelControl`. Панель одна на приложение (`sharedPreviewPanel`).
- Предпросмотр нужен выделенной строке; у стопки подписей Корзины выделения нет, и переводить её на таблицу ради
  пробела — отдельное решение, в эту дельту не входит.
- `UNUserNotificationCenter.currentNotificationCenter` бросает исключение в процессе без бандла — проверка
  `bundleIdentifier` обязательна до обращения. Запрос разрешения — системный диалог один раз; отказ пользователя
  не ошибка.
- `NSDockTile.contentView` рисуется только из бандла и только пока плитка видна; после `clear` вызывается `display`.
- «Music ▸ Music»: сравнение имён узла и родителя в `DiagramTree`, без обращения к индексу — работает и для
  измеренных узлов.
- Открыть каталог в Terminal: `openURLs:withApplicationAtURL:configuration:completionHandler:` с URL каталога;
  Terminal открывает окно в нём. Для файла — родительский каталог.
- CSV: UTF-8 без BOM, разделитель запятая, кавычки удваиваются; байты — число, не форматированный размер (единицы
  решения 17 относятся к интерфейсу, не к экспорту).
- Ограничение проверяемости (честно): открытие панели Quick Look, полоса в Dock, системное уведомление и
  перетаскивание в System Settings живут только в подписанном бандле с окном — исполняемые критерии доказывают
  модели (цели предпросмотра, геометрию полосы, строку уведомления, выборку путей) и сборку реальных модулей в
  `wire`; остальное — приёмка человеком после `wire`.

# Gameplan

### Phase 1: sunburst-group — группа «мелкие объекты» в кольце

<!-- deps: [] -->

- [ ] `render/src/segment_kind.rs`, `small_groups.rs`, `hit.rs`; `ArcSegment.kind`; `SunburstLayout::compute` с
  `SmallGroups`, сбор остатка в группу, раскрытие; `hit_test -> Option<Hit>`; `SMALL_GROUP_SPAN` публичная.
- [ ] `diagram_state.rs`: `small_groups`, клик и Enter по группе, `set_small_groups_always`; `accessibility_model.rs`
  и `diagram_strings.rs`: элемент группы; `diagram_painter.rs`: цвет группы токеном.
- [ ] `app`: `diagram_preference.rs` ключ `ExpandSmallGroups`, меню «Вид», `ShellAction::ToggleSmallGroups`,
  `shell_controller.rs` передаёт настройку в `DiagramState`.
- [ ] `render/tests/layout.rs`: кольцо сходится при группе, группа одна на каталог, узкие дуги только в группе,
  `hit_test` середины группы → `Hit::Group(parent)`, раскрытая группа кладёт членов, сектор скана не ломается.
  `render/tests/accessibility.rs`: элемент группы с подписью и кадром; клавиатурный обход включает группу.
- [ ] `docs/DOMOVOI.md` 4.3, книга экран 03.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-render --test layout
```
- Сходимость, одна группа на каталог, hit-test группы, раскрытие (exit 0).
```
cargo test -p domovoi-render --test accessibility
```
- Группа доступна VoiceOver и клавиатуре (exit 0).
```
cargo test -p domovoi-app --test screen_update
```
- Обновление «Диска» с новой раскладкой не ломает перенумерацию узлов (exit 0).
```
cargo xtask check-tokens
```
- Цвет группы — токен темы (exit 0).
```
cargo clippy -p domovoi-render -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: names-menu-csv — имя с родителем, меню узла, экспорт CSV

<!-- deps: [] -->

- [ ] `app/src/nested_name.rs`; применение в `level_rows`, `HoverTip`, центре диаграммы.
- [ ] `app/src/node_facts.rs`, `properties_panel.rs`; пункты «Открыть в Terminal» и «Свойства» в `level_list.rs`
  и в меню сегмента диаграммы; `ShellAction::OpenInTerminal`, `ShowProperties`; `shell_actions.rs` селекторы.
- [ ] `app/src/csv_export.rs`; «Файл ▸ Экспортировать список…» в `main_menu.rs`, `NSSavePanel` в
  `shell_controller.rs`; `ShellAction::ExportCsv`.
- [ ] `app/tests/nested_name.rs` (новый): «Music ▸ Music» для совпадающих имён, обычное имя иначе, измеренные узлы.
  `app/tests/csv_export.rs` (новый): заголовок, экранирование кавычек и запятых, порядок обхода, байты числом,
  на индексе из `TempTree`. `app/tests/node_facts.rs` (новый): даты и число записей из индекса.
- [ ] `strings.rs` новые строки; `docs/DOMOVOI.md` 4.8; книга экран 03.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test nested_name
```
- Имя с родителем (exit 0).
```
cargo test -p domovoi-app --test csv_export
```
- Экспорт по RFC 4180 на реальном индексе (exit 0).
```
cargo test -p domovoi-app --test node_facts
```
- Свойства узла из индекса (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые строки без восклицаний и запрещённых фраз (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: quick-look — предпросмотр по пробелу в списке уровня и дубликатах

<!-- deps: [] -->

- [ ] `Cargo.toml`: `objc2-quick-look-ui = "=0.3.2"`; `app/Cargo.toml` фичи по компилятору.
- [ ] `duplicates_source.rs`: выделение строк участников.
- [ ] `app/src/preview_targets.rs`, `preview_source.rs`, `preview_table.rs`; `ShellRootView` отдаёт панель;
  `ShellAction::TogglePreview`; пробел в списке уровня и дубликатах.
- [ ] `app/tests/preview_targets.rs` (новый): выборка URL из выделения двух моделей, пустое выделение → нет целей,
  строки без пути (измеренные узлы) пропускаются, заголовок группы дубликатов не цель.
- [ ] `docs/DOMOVOI.md` 4.8; книга экран 06.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test preview_targets
```
- Цели предпросмотра из выделения (exit 0).
```
cargo test -p domovoi-app --test snapshot_shell
```
- Снимки всех экранов с выделяемыми дубликатами держат размеры и детерминизм (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: dock-notice — прогресс в Dock и уведомление о завершении

<!-- deps: [] -->

- [ ] `Cargo.toml`: `objc2-user-notifications = "=0.3.2"`; `app/Cargo.toml` фичи `NSDockTile`, уведомлений.
- [ ] `app/src/dock_progress.rs` (`DockProgressBar`, геометрия полосы `bar_frame(sweep, size)` чистой функцией),
  `scan_notice.rs`, `notification_center.rs` (`ScanNotifier`); `scan_tick`/завершение в `shell_controller.rs`;
  `app_delegate.rs` — очистка плитки при завершении.
- [ ] `app/tests/dock_progress.rs` (новый): ширина полосы по `sweep` 0, 0,5, 1; `app/tests/voice.rs`: строка
  «Скан завершён за 2 мин 10 с» и «за 48 с».
- [ ] `docs/DOMOVOI.md` 4.8; книга раздел VIII.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test dock_progress
```
- Геометрия полосы (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Строка завершения по голосу интерфейса (exit 0).
```
cargo test -p domovoi-app --test rescan
```
- Повторный скан с плиткой и уведомителем не ломает цикл скана (exit 0).
```
cargo xtask check-tokens
```
- Полоса Dock — токены темы (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 5: access-drag — иконка для перетаскивания на экране доступа

<!-- deps: [] -->

- [ ] `app/src/app_icon_drag.rs` (`AppIconView`, `NSDraggingSource`, `NSPasteboardItem` с URL бандла);
  `access_screen.rs` — иконка у шага 3, `ACCESS_STEPS[2]`, `ACCESS_DRAG_HINT`; `Probes` без изменений.
- [ ] `app/tests/snapshot_shell.rs`: экран 01 остаётся ≤ 600 pt (без правок теста); `app/tests/voice.rs`: новые
  строки.
- [ ] `docs/DOMOVOI.md` 4.8 и 6.3 (перетаскивание), книга экран 01.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test snapshot_shell
```
- Экран доступа с иконкой держит высоту (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Текст шага без восклицаний (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 6: wire — сквозной путь через бандл

<!-- deps: [sunburst-group, names-menu-csv, quick-look, dock-notice, access-drag] — потребление: сценарий собирает настоящие SunburstLayout с SmallGroups, nested_name, csv_export, preview_targets, dock_progress и ScanNotifier. -->

- [ ] `app/src/scenario_step.rs`, `end_to_end.rs`: шаги `ExportCsv { path }` (пишет CSV реального индекса через
  `csv_export`), `PreviewTargets { level_selection }` (отчёт числа целей), `SmallGroups` (отчёт: число групп в
  кольце корня, сумма углов кольца 0), `ScanNotice` (строка завершения по фактическому времени скана).
- [ ] `app/tests/wire_main_path.rs`: сценарий на бандле `cargo xtask bundle --profile adhoc` — скан `TempTree` с
  сотней мелких файлов в одной папке и двумя одноимёнными вложенными папками; утверждения: кольцо сходится, ровно
  одна группа у корня, строка уровня «Music ▸ Music», CSV существует и число строк равно числу узлов индекса,
  цели предпросмотра по выделению > 0, строка «Скан завершён за …» без восклицаний.
- [ ] Граница приёмки человеком: панель Quick Look по пробелу, полоса в Dock во время скана, уведомление при
  неактивном окне, перетаскивание иконки в список Full Disk Access.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь через бандл с реальными модулями всех пяти фаз (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-tokens
```
- Цвета только в теме (exit 0).
```
cargo clippy -p domovoi-render -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
