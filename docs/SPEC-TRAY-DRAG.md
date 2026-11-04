<!--
  SPEC-TRAY-DRAG.md — дельта к docs/SPEC-DOMOVOI.md: строки списка уровня перетаскиваются на кнопку «К удалению» в
  шапке главного экрана, кнопка показывает число и объём собранного, а в листе «К удалению» любую строку можно снять
  с отметки. 4 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 04.11.2026, выбран вариант A по рекомендации): перетаскивание только внутри приложения —
  свой тип pasteboard com.domovoi.node, приёмник — кнопка «К удалению» с вердиктом denylist над ней и строкой отказа
  после броска. Отвергнуты: B — file URL в pasteboard (строку, брошенную в Finder, тот же том перемещает, другой —
  копирует десятки гигабайт; источник расширяется до Finder вопреки решению «только строки списка»); C — строку под
  защитой нельзя начать тащить (молчаливый отказ, причина только в подсказке; вердикт на каждую строку при
  отрисовке; смешанное выделение не тянется целиком). Корзина Finder не возвращается — решение пользователя до веера.
  На обзоре спеки метка «к удалению» на строке списка уровня отложена: её делать вместе со сжатием сегмента
  диаграммы, отдельной задачей.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-PURGE-SPACE.md`
  (влита в `main`, `7dc1ba8`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Произвольную папку главного экрана в «К удалению» не положить: `Session::add_to_tray`
  (`crates/app/src/session.rs:328`) зовут только карточки рекомендаций, копии дубликатов и ответы ассистента, у строки
  уровня есть лишь «Удалить вместе с файлами» для бандла. Ошибочную отметку не снять: в листе
  (`crates/app/src/tray_screen.rs`) у строк нет кнопок, «Отмена» только закрывает лист, хотя `Tray::remove`
  (`crates/rules/src/tray.rs`) и `Session::remove_from_tray` есть.
- Уже есть и не меняется: список сбора `Tray` (`add`, `add_application`, `remove`, `take_for_delete` со сверкой
  denylist в момент взятия), лист `TraySheet` / `TrayScreen` на `TrayColumn`, замер `TrayMeasurement` (новый список
  путей — новый замер; `Drop` гасит таймер и отменяет задачу), подтверждение `confirm_delete` (NSAlert), движок
  `crates/rules/src/delete_run.rs`, прогресс «Удаляю: N из M файлов», отчёт, `ShellGate`.
- Не меняется решение `docs/SPEC-PERMANENT-DELETE.md`: удаление окончательное, Корзина Finder не используется.
- Вне этой спеки: «сжатие сегмента при отметке к удалению 320 мс» (`docs/DOMOVOI.md`, книга, раздел IV) — в
  `crates/render` нет состояния «отмечен», это отдельная задача; метка отмеченной строки в списке уровня (делается
  вместе со сжатием сегмента); перетаскивание с диаграммы; пункт меню «Отметить к удалению»; бросок файлов из Finder.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч; `.github/workflows/ci.yml`
  версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/rules/src/tray.rs`: `Tray::covers(&self, path: &Path) -> bool` — путь равен пути элемента или лежит внутри
  него (условие, по которому `insert` пропускает узел).
- `crates/app/src/drop_refusal.rs` (новый): `DropRefusal { name: String, reason: DenyReason }`.
- `crates/app/src/drop_kind.rs` (новый): `DropKind { Add, Denied, Already }`.
- `crates/app/src/drop_hover.rs` (новый): `DropHover { kind: DropKind, title: String }`.
- `crates/app/src/drop_verdict.rs` (новый): `DropVerdict { allowed: Vec<NodeId>, refused: Vec<DropRefusal>,
  already: usize, allowed_bytes: u64 }`; `judge(index, denylist, tray, paths)`, `hover() -> DropHover`,
  `refusal_line() -> Option<String>`.
- `crates/app/src/tray_summary.rs` (новый): `TraySummary { objects: usize, bytes: u64 }`, `title() -> String`.
- `crates/app/src/session.rs`: `judge_drop(&self, paths: &[PathBuf]) -> DropVerdict`,
  `drop_into_tray(&mut self, paths: &[PathBuf]) -> DropVerdict`, `tray_summary(&self) -> TraySummary`.
- `crates/app/src/strings.rs`: `DROP_ADD` «Добавить», `DROP_DENIED` «Нельзя:», `DROP_ALREADY` «Уже в списке»,
  `NOT_ADDED` «Не добавлено:», `UNMARK` «Снять отметку», новый текст `TRAY_EMPTY`.
- `crates/app/src/tray_model.rs`: `row_nodes: Vec<Vec<NodeId>>` (`#[serde(skip)]`) — узлы каждой строки в порядке
  `rows`.
- `crates/app/src/tray_screen.rs`: крестик в строке подтверждения, селектор `unmarkTrayRow:`, тег — номер строки.
- `crates/app/src/shell_action.rs`, `shell_actions.rs`: `UnmarkTrayRow(usize)`, `DragFromList`, `DragEnded`,
  `DropOnTray`; селектор `unmarkTrayRow:`.
- `crates/app/src/level_row.rs`: `draggable: bool`; `shell_data.rs` ставит его строкам с `NodeOrigin::Indexed`.
- `crates/app/src/node_pasteboard.rs` (новый): тип `com.domovoi.node`.
- `crates/app/src/level_list_source.rs`: `tableView:pasteboardWriterForRow:`,
  `tableView:draggingSession:willBeginAtPoint:forRowIndexes:`, `tableView:draggingSession:endedAtPoint:operation:`,
  `dragged_rows()`; `crates/app/src/level_list.rs`: `setAllowsMultipleSelection(true)`, маска операций источника.
- `crates/app/src/tray_drop_button.rs` (новый): кнопка-приёмник — рисуется как `ShellButton`,
  `NSDraggingDestination` для `com.domovoi.node`, состояния покоя, ожидания, «добавить», отказа и «уже в списке».
- `crates/app/src/main_screen.rs`: `tray: Retained<TrayDropButton>`, строка отказа под кнопкой, `show_tray_summary`.
- `crates/app/src/shell_state.rs`: снимок перетаскивания `drag: Option<Vec<PathBuf>>`, таймер строки отказа.
- `crates/app/src/shell_controller.rs`: обработчики `DragFromList`, `DragEnded`, `DropOnTray`, `UnmarkTrayRow`; сводка
  на кнопке после каждого изменения списка.
- `crates/app/src/scenario_step.rs`, `end_to_end.rs`: шаги `mark_from_list { paths }`, `unmark_tray_row { row }`.
- `docs/DOMOVOI.md`, `design/domovoi-design-book.html`: перетаскивание, состояния кнопки, снятие отметки.

# Conventions

- Один тип — один файл; имена без сокращений; строки интерфейса только в `strings.rs` и проходят `app/tests/voice.rs`.
- Цвета только из токенов палитры (`cargo xtask check-tokens`); комментарии не ссылаются на спеку и фазы
  (`cargo xtask check-comments`).
- Список «К удалению» меняется только через методы `Session`; запрет — только `Denylist::verdict`.
- Новый тест с AppKit — `harness = false` и запись `[[test]]` в `crates/app/Cargo.toml`, как у `truncation`.
- Typecheck-критерий — `cargo clippy -p <crate> --all-targets -- -D warnings`.

# Knowledge

- **Источник перетаскивания.** Тянутся только строки списка уровня с узлом индекса (`NodeOrigin::Indexed`); для
  синтетических строк («Скрытое», «Другие тома», сборная строка мелких) писатель pasteboard возвращает `nil`, и AppKit
  их не тянет. Выделение множественное; перетаскивание невыделенной строки берёт только её (поведение `NSTableView`).
  Pasteboard несёт только свой тип `com.domovoi.node` со строкой пути; маска операций источника — `Copy` внутри
  приложения и пустая снаружи, поэтому ни Finder, ни другая программа бросок не примут.
- **Снимок и вердикт.** В `willBeginAtPoint` источник запоминает строки и шлёт `DragFromList`; контроллер переводит
  строки в пути (`indexed_node` → `Index::path`), хранит снимок в `ShellState::drag`, считает `Session::judge_drop` и
  вооружает кнопку заголовком из `DropHover`. Вердикт по каждому пути: узел ищется через `Index::find`, нет узла — путь
  пропускается; вердикт `Denylist::verdict` — отказ с причиной (`deny_reason_text`); `Tray::covers` — «уже в списке»;
  иначе разрешён. Объём разрешённых — сумма `reclaimable_estimate`.
- **Бросок.** Над кнопкой с вердиктом `Add` приёмник отвечает `Copy`, с `Denied` и `Already` — `None`.
  `performDragOperation` шлёт `DropOnTray`: контроллер повторяет вердикт по снимку на текущем индексе (индекс мог
  обновиться за время перетаскивания — поэтому снимок хранит пути), кладёт разрешённые через `drop_into_tray` без
  `rule_id`, показывает строку отказа, если отказы есть, и обновляет сводку. `endedAtPoint` шлёт `DragEnded`: снимок
  стирается, кнопка возвращается в покой; если операция `None`, точка конца внутри кнопки и вердикт `Denied` —
  показывается строка отказа.
- **Заголовки кнопки.** Покой: «К удалению» при пустом списке, «К удалению · 3 · ≈ 42,2 ГБ» при непустом
  (`TraySummary`: число элементов и сумма `reclaimable_estimate`, `≈` — `ESTIMATE_MARK`; точный объём — в листе после
  замера). Над кнопкой: все разрешены — «Добавить 2 объекта · ≈ 36,0 ГБ» (`format_count` по `OBJECTS`); часть
  отсеяна — «Добавить 2 из 3 · ≈ 90,5 ГБ»; разрешённых нет, есть отказы — «Нельзя: системный том» (разные причины
  через запятую); разрешённых и отказов нет — «Уже в списке». Ширина кнопки следует заголовку (`retitle`), шапка
  раскладывается заново (`lay_out_head`), крошки сжимаются.
- **Строка отказа.** «Не добавлено: System — системный том · usr — системный том» — под кнопкой, правый край по кнопке,
  шрифт интерфейса 12, цвет `risk`; гаснет через 4 с (`once`) или в начале следующего перетаскивания.
- **Вид состояний.** Ожидание (идёт перетаскивание из списка): кольцо 2 pt цвета `wax` с прозрачностью 0,55 в 3 pt от
  кнопки, без анимации — бюджет движения книги не пополняется. «Добавить»: кольцо `wax` полной яркости, заливка `wax`,
  текст `wax_ink`. Отказ: заливка `surface_2`, текст и кольцо `risk`. «Уже в списке»: заливка `surface_2`, текст
  `mute`, кольцо `line`. Покой — как сейчас.
- **Снятие отметки.** В состоянии подтверждения у каждой строки листа справа от размера крестик «×» — `ShellButton` без
  заливки, текст `dim`, подсказка и подпись доступности «Снять отметку: <заголовок строки>», тег — номер строки.
  Крестик снимает все узлы строки (`TrayModel::row_nodes`): отдельный узел; группу правила целиком; программу с её
  файлами целиком (`Tray::remove` убирает опустевшую `TrayApplication`). Пути «Не отмечено — совпадает только имя»
  элементами не являются и не затрагиваются. Во время удаления, в итоге и в пустом листе крестиков нет. Это
  единственный способ снять отметку.
- **После снятия.** Лист строится заново через `measured_tray`: список путей изменился, поэтому начинается новый замер
  оставшихся строк с оценкой («≈» и «Уточняю размер»), прежний гасит `Drop` у `TrayMeasurement`. Снята последняя
  строка — пустой лист с «Закрыть». Сводка на кнопке главного экрана обновляется.
- **Пустой лист.** `TRAY_EMPTY`: «Пока ничего не отмечено к удалению. Перетащите строки списка на кнопку
  «К удалению», отметьте рекомендации или копии дубликатов.»
- **Предел проверки.** Жест мыши, рисунок перетаскивания AppKit и отказ Finder принять свой тип тестами не
  воспроизводятся: тесты проверяют вердикт, писатель pasteboard, состояния кнопки прямыми вызовами и путь броска через
  сессию в сценарии бандла. Перетаскивание мышью на бандле — приёмка человеком на границе фазы `wire`.

# Gameplan

<!-- 4 фазы цепочкой; wire — сквозной сценарий бандла и документы. -->

### Phase 1: drop-verdict — вердикт броска и сводка списка

- [ ] `Tray::covers` в `crates/rules/src/tray.rs`; случаи в `crates/rules/tests/tray_delete.rs`: тот же путь, путь
  внутри элемента, соседний путь с общим началом имени (`/a/b` и `/a/bc`) не покрыт.
- [ ] `drop_refusal.rs`, `drop_kind.rs`, `drop_hover.rs`, `drop_verdict.rs`, `tray_summary.rs` по Stack и Knowledge;
  экспорт из `lib.rs`.
- [ ] `Session::judge_drop`, `drop_into_tray`, `tray_summary`.
- [ ] Строки `DROP_ADD`, `DROP_DENIED`, `DROP_ALREADY`, `NOT_ADDED` в `strings.rs`; фразы — в `app/tests/voice.rs`.
- [ ] `app/tests/tray_drop.rs` (новый) на `ImageVolume` через `Session::open`: разрешённая папка — `Add` и заголовок
  с оценкой; папка с `chflags uchg` — `Denied`, «Нельзя: защищено системой» и строка отказа; папка внутри отмеченной —
  `Already`; смешанный бросок — «Добавить 1 из 2 · ≈ …»; `drop_into_tray` кладёт только разрешённые и без `rule_id`;
  отметка родителя вытесняет отмеченного потомка; путь вне индекса пропускается; сводка пустого и непустого списка.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test tray_delete
```
- `Tray::covers` различает вложенный путь и соседа с общим началом имени (exit 0).
```
cargo test -p domovoi-app --test tray_drop
```
- Вердикт, заголовки, строка отказа и сводка на настоящем индексе образа (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые фразы без восклицаний и запрещённых слов (exit 0).
```
cargo clippy -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы для `domovoi-rules` (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы для `domovoi-app` (exit 0).

### Phase 2: unmark — снятие отметки в листе «К удалению» (order-only после фазы 1)

- [ ] `TrayModel::measured` заполняет `row_nodes`: строка без группы — один узел, группа правила — все узлы правила,
  группа программы — все узлы программы.
- [ ] `tray_screen.rs`: крестик у строки в состоянии подтверждения (не во время удаления, не в итоге, не в пустом
  листе), подсказка и подпись доступности «Снять отметку: <заголовок>»; заголовок строки сужается на ширину крестика.
- [ ] `ShellAction::UnmarkTrayRow(usize)` и селектор `unmarkTrayRow:`; контроллер снимает узлы строки через
  `Session::remove_from_tray` и показывает лист заново через `measured_tray`.
- [ ] Строка `UNMARK` и новый `TRAY_EMPTY` в `strings.rs`; фразы — в `app/tests/voice.rs`.
- [ ] `app/tests/tray_unmark.rs` (новый): узлы строк для отдельного узла, группы правила и группы программы; снятие
  группы программы убирает `TrayApplication`; снятие последней строки даёт пустую модель; после снятия
  `TrayModel::measured` строится по оставшимся элементам.
- [ ] `app/tests/truncation.rs`: `TrayScreen` с крестиками и длинным заголовком строки — заголовок обрезается, крестик
  и размер видны целиком.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test tray_unmark
```
- Строка листа знает свои узлы, снятие убирает строку, группу и опустевшую программу (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Лист с крестиками не обрезает размер и крестик (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые фразы без восклицаний и запрещённых слов (exit 0).
```
cargo test -p domovoi-app --test snapshot_shell
```
- Экраны по-прежнему рисуются в обеих темах (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: drag-ui — перетаскивание строк на кнопку «К удалению» (потребляет фазу 1: DropVerdict, Session::judge_drop, drop_into_tray, TraySummary)

- [ ] `level_row.rs`: `draggable`; `shell_data.rs` ставит его строкам с `NodeOrigin::Indexed`.
- [ ] `node_pasteboard.rs`; `level_list_source.rs`: писатель pasteboard только для `draggable` строк, начало и конец
  перетаскивания, `dragged_rows()`; `level_list.rs`: множественное выделение и маска операций источника.
- [ ] `tray_drop_button.rs`: рисунок как у `ShellButton`, приём `com.domovoi.node`, состояния по Knowledge, `arm`,
  `disarm`, `show_summary`; бросок шлёт `ShellAction::DropOnTray`.
- [ ] `main_screen.rs`: кнопка-приёмник вместо `ShellButton`, строка отказа под ней, раскладка шапки после смены
  заголовка.
- [ ] `ShellAction::DragFromList`, `DragEnded`, `DropOnTray`; `shell_state.rs`: снимок путей и таймер строки отказа;
  контроллер: вердикт в начале, перепроверка при броске, строка отказа на 4 с через `once`, сводка на кнопке после
  каждого изменения списка (рекомендации, дубликаты, ассистент, программа с файлами, снятие, бросок, удаление).
- [ ] `app/tests/tray_drop_button.rs` (новый, `harness = false`): кнопка, вооружённая настоящим `DropVerdict` из
  `Session::judge_drop` на `ImageVolume`, показывает заголовок и вид каждого `DropKind`; `show_summary` с настоящей
  `TraySummary`; писатель pasteboard `LevelListSource` даёт элемент типа `com.domovoi.node` для индексированной строки
  и ничего для синтетической.
- [ ] `app/tests/truncation.rs`: шапка главного экрана с самым длинным заголовком кнопки — крошки сжимаются, кнопка,
  «свободно» и переключатель не перекрываются.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test tray_drop_button
```
- Кнопка на настоящем вердикте фазы 1 и писатель pasteboard списка (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Шапка с длинным заголовком кнопки не перекрывается (exit 0).
```
cargo test -p domovoi-app --test snapshot_shell
```
- Главный экран по-прежнему рисуется в обеих темах (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: wire — перетаскивание и снятие на главном пути бандла (потребляет фазы 1-3)

- [ ] `scenario_step.rs` / `end_to_end.rs`: шаг `mark_from_list { paths }` — путь обработчика броска:
  `Session::judge_drop`, затем `drop_into_tray`, запись заголовка над кнопкой, строки отказа и сводки; шаг
  `unmark_tray_row { row }` — узлы строки из `TrayModel::row_nodes` через `Session::remove_from_tray`, запись строк
  модели и сводки.
- [ ] `app/tests/wire_main_path.rs`: на свежих папках образа после шагов удаления — бросок разрешённой папки и папки с
  `chflags uchg`: одна добавлена, отказ «защищено системой», сводка «К удалению · 1 · ≈ …»; снятие строки 0 — модель
  пуста, сводка «К удалению».
- [ ] Пересобрать бандл `cargo xtask bundle --profile adhoc` перед прогоном.
- [ ] `docs/DOMOVOI.md`: абзац главного экрана (перетаскивание строк, кнопка-приёмник, строка отказа), раздел
  «К удалению» (снятие отметки, текст пустого листа), экран 8; `design/domovoi-design-book.html`: состояния кнопки в
  разделе VI, крестик в строке экрана 08, фразы в разделе VIII.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Бросок и снятие проходят на бандле через настоящие модули фаз 1-3 (exit 0).
```
cargo xtask check-sources
```
- Документ и книга не расходятся с принятыми решениями (exit 0).
```
cargo xtask check-comments
```
- Комментарии не ссылаются на спеку и фазы (exit 0).
```
cargo xtask check-tokens
```
- Цвета новых видов только из токенов (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
