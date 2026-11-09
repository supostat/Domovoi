<!--
  FIX-EMPTY-PROJECT-ROW.md — дельта к docs/SPEC-DOMOVOI.md: строку проекта на 0 Б в карточке «Проекты разработки»
  отмечают галочки группы и карточки, и она уходит в «К удалению»; после изменения на диске отметка снимается сама,
  и карточка становится отмеченной частично. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, три гипотезы, выбрана первая): правило «строку на 0 Б не отмечают» живёт в виде
  и в keep_selection, а переключатели CardModel его не знают; правка — одно правило ProjectRow::selectable для
  переключателей, состояния карточки и группы, переноса отметок и галочек вида.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-REBUILD-WITHOUT-SESSION.md` (`6df293a` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. В карточке «Проекты разработки» у проекта, который ничего не освобождает (0 Б), галочка строки неактивна,
  но галочка группы «Без изменений больше месяца» и галочка карточки отмечают и его; проект уходит в «К удалению».
  После следующего изменения на диске `keep_selection` снимает с него отметку, и карточка становится отмеченной
  частично.
- Уже есть и не меняется: строка на 0 Б остаётся в карточке с неактивной галочкой (так главный путь показывает
  `frozen`); карточки правил на 0 Б не отмечаются через `CardModel::selectable`; лоток, группы и порядок строк.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/app/src/project_row.rs`: `ProjectRow::selectable` — строку можно отметить, только если она что-то
  освобождает (`amount > 0`).
- `crates/app/src/card_model.rs`: `toggle_row`, `toggle_group`, `toggle_rows` меняют только строки, которые можно
  отметить; `every_row_selected` — есть строка, которую можно отметить, и отмечены все такие строки; «группа отмечена
  целиком» считает один метод модели, его зовут и `toggle_group`, и заголовок группы.
- `crates/app/src/recommendations_model.rs`, `keep_selection`: перенос отметки строки через `ProjectRow::selectable`.
- `crates/app/src/project_rows.rs`: галочка строки активна по `ProjectRow::selectable`; галочка группы активна, если
  в группе есть такая строка, а «отмечена» и смешанное состояние берёт из метода модели.
- `crates/app/tests/empty_project_row.rs` (написан диагнозом): регрессия на модели карточки.
- `docs/DOMOVOI.md`, абзац «Проекты разработки»: проект на 0 Б не отмечается ни строкой, ни группой, ни карточкой.

# Conventions

- Одно правило «строку можно отметить» — `ProjectRow::selectable`; сравнение `amount > 0` для строк проекта больше
  нигде не повторяется.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-app --all-targets -- -D warnings`; формат — `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test empty_project_row` — 0 passed, 4 failed:
    the_group_check_leaves_a_project_that_frees_nothing_unchecked:
      left: [("quartz", true), ("facet", true), ("clavis", true), ("engram", false), ("boss", false)]
     right: [("quartz", true), ("facet", false), ("clavis", true), ("engram", false), ("boss", false)]
    the_row_check_of_a_project_that_frees_nothing_does_nothing:
      assertion failed: !card.rows[position].selected
    the_card_check_selects_every_project_that_frees_space_and_checks_the_card:
      left: [("quartz", true), ("facet", true), ("clavis", true), ("engram", true), ("boss", true)]
     right: [("quartz", true), ("facet", false), ("clavis", true), ("engram", true), ("boss", true)]
    a_rebuild_keeps_the_selection_made_by_the_card_check:
      left: [("quartz", true), ("facet", false), ("clavis", true), ("engram", true), ("boss", true)]
     right: [("quartz", true), ("facet", true), ("clavis", true), ("engram", true), ("boss", true)]
- **Причина.** Правило «строку на 0 Б не отмечают» есть только в виде (`project_rows.rs:79` — галочка группы,
  `project_rows.rs:135` — галочка строки) и в переносе отметок (`recommendations_model.rs:91`); `toggle_rows`,
  `toggle_row`, `toggle_group` (`card_model.rs:104-129`) ставят отметку каждой строке. При следующей перестройке
  `keep_selection` снимает отметку со строки на 0 Б — отсюда «сама снимается» и смешанная карточка.
- **Ловушка.** Запретить только отметку строки мало: `every_row_selected` (`card_model.rs:96`) и «всё отмечено» в
  заголовке группы (`project_rows.rs:73`) считают все строки — карточка и группа навсегда останутся смешанными, а
  повторный клик по карточке снова отметит строки вместо снятия.
- **Откуда цели у строки на 0 Б.** Найденный артефакт с 0 освобождаемых байт (место держат клоны или снимки)
  ложится в `items` проекта (`claim_ledger.rs:276`), поэтому отмеченная строка несёт цели в лоток.
- **Отвергнуто.** Убрать строку на 0 Б из карточки — главный путь намеренно показывает `frozen` (0 Б) строкой
  (`wire_main_path.rs:121-125`), а карточки правил на 0 Б тоже видны неактивными. Отсечь строку только в
  `selected_amount`/`selected_targets` — галочка по-прежнему врёт, и отметка так же снимается сама.
- **Проверка мутацией.** Одна мутация на прогон: вернуть в `toggle_group` отметку всех строк группы —
  `empty_project_row` краснеет.

# Gameplan

<!-- 1 фаза: одно правило отметки строки проекта. -->

### Phase 1: empty-project-row — проект на 0 Б нельзя отметить никаким способом

- [ ] `ProjectRow::selectable` и его применение по Stack: переключатели и состояние карточки и группы в `CardModel`,
  `keep_selection`, галочки строки и группы в `project_rows.rs`.
- [ ] Абзац «Проекты разработки» в `docs/DOMOVOI.md`: проект на 0 Б не отмечается ни строкой, ни группой, ни
  карточкой.
- [ ] Оставить `crates/app/tests/empty_project_row.rs` регрессионным тестом; одна мутация по Knowledge.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test empty_project_row
```
- Проект на 0 Б не отмечается строкой, группой и карточкой; карточка отмечается целиком; отметки переживают перестройку (exit 0).
```
cargo test -p domovoi-app --test projects_card
```
- Отметка строк, групп и карточки проектов и её перенос не изменились (exit 0).
```
cargo test -p domovoi-app --test selection_after_rebuild
```
- Перенос отметок карточек правил и гипотез не изменился (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
