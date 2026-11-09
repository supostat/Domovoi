<!--
  FIX-CLONED-COPIES.md — дельта к docs/SPEC-DOMOVOI.md: в «Дубликатах» копии, которые клонируют друг друга, но не
  оставляемый файл, показаны «клон APFS» без галочки. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1, правка C из трёх после ревью черновика): общий поток или инод
  проверяется против любого члена группы; правка — копию блокирует только общее с оставляемым. Счёт блоков, которые
  делят удаляемые вместе файлы, — отдельная задача общей модели reclaimable.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-REINSTALLED-OWNER.md` (`58428c4` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Группа из трёх одинаковых файлов, где две копии — клоны APFS друг друга, но не оставляемого файла: обе
  копии показаны «клон APFS» без галочки, их нельзя отметить и удалить.
- Уже есть и не меняется: выбор оставляемого экземпляра; копия, которая делит поток или инод с оставляемым, помечена
  «клон APFS», не отмечается и ничего не освобождает; копии внутри пакетов; замена клоном и её отказы; вид строк
  группы; модель reclaimable (клон, чьи блоки держит другой файл, даёт `held_by_clones`) и замер листа «К удалению».
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/dups/src/duplicate_copy.rs`: поле `shares_blocks` → `shares_with_kept` — копия на том же REALDEVID, что
  оставляемый, с тем же FILEID или тем же ненулевым CLONEID; `is_removable` читает его.
- `crates/dups/src/duplicate_group.rs`: `shares_blocks_with_another_member` заменяется сравнением копии с оставляемым
  (`shares_blocks_with_kept`); выигрыш группы считается как прежде — по копиям, которые можно удалить.
- `crates/app/src/duplicates_model.rs`: `clone: copy.shares_with_kept`.
- Переименование поля в `crates/dups/tests/pipeline.rs`, `clone_replace_image.rs`, `smoke_rules.rs`.
- `crates/dups/tests/clones_among_copies.rs` (написан диагнозом): регрессия на группе.
- `docs/DOMOVOI.md`, 4.5: строка о клонах — помечается и не отмечается копия, которая делит блоки с оставляемым
  экземпляром (клон или жёсткая ссылка); копии, которые делят блоки только между собой, отмечаются как обычные, а их
  общие блоки в «освободит» не засчитываются.

# Conventions

- Одно правило «копию блокирует общее с оставляемым» — в `dups`; экран берёт `shares_with_kept` и не сравнивает
  потоки сам.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-dups -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-dups --test clones_among_copies` — 0 passed, 1 failed:
    copies_that_are_clones_only_of_each_other_can_go (clones_among_copies.rs:73:5):
      left: {"…/Backup/report.raw": false, "…/Old/report.raw": false}
     right: {"…/Backup/report.raw": true, "…/Old/report.raw": true}
- **Причина.** `shares_blocks_with_another_member` (`duplicate_group.rs:83-90`) поднимает флаг при общем FILEID или
  CLONEID с любым другим членом группы, в том числе с другой копией; заметка 12e492e0 записала это правило как
  исходное. Удалять бессмысленно только копию, чьи блоки держит оставляемый.
- **Остаётся.** `pipeline.rs:141-155`: клоны и жёсткие ссылки оставляемого — `shares_blocks`, независимые копии — нет;
  `clone_replace_image.rs:181` (existing_clones_have_no_gain): клоны оригинала без выигрыша.
- **Счёт пары.** Каждая из двух копий-клонов отдельно — `Reclaimable { bytes: 0, held_by_clones: 2097152 }` (вывод
  диагноза). Модель reclaimable не засчитывает блоки, которые делят удаляемые вместе файлы, нигде: `split_private`
  при `private_size = 0` относит всё в `held_by_clones` и в замере папки с обоими клонами (`reclaimable.rs:67-77`),
  карточки — `estimate_file` по `shares_with + 1` всего индекса (`reclaimable.rs:43-55`). Поэтому «освободит» пары
  остаётся 0 Б, а место тома после удаления пары растёт.
- **Отвергнуто.** Правка A — семейства копий со своим счётом в `dups` и на экране: третье правило подсчёта рядом с
  моделью и листом, переписывается при общем учёте. Правка B — семейство одной строкой: то же плюс смена вида.
  Включить учёт между пунктами листа в этот фикс — корень в модели reclaimable, а не в листе.
- **Не входит.** Общий учёт «поток, все файлы которого удаляются вместе, освобождает свои блоки один раз» в индексе,
  листе «К удалению», рекомендациях и «Дубликатах» — отдельная задача через `/mneme:plan`.
- **Проверка мутацией.** Одна мутация на прогон: вернуть сравнение со всеми членами группы —
  `clones_among_copies` краснеет.

# Gameplan

<!-- 1 фаза: копию блокирует только общее с оставляемым. -->

### Phase 1: cloned-copies — клоны среди копий можно отметить и удалить

- [ ] `DuplicateCopy::shares_with_kept` и сравнение с оставляемым в `DuplicateGroup::of_members` по Stack;
  переименование поля в тестах `dups` и в `duplicates_model.rs`.
- [ ] Строка о клонах в 4.5 `docs/DOMOVOI.md`.
- [ ] Оставить `crates/dups/tests/clones_among_copies.rs` регрессионным тестом; одна мутация по Knowledge.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-dups --test clones_among_copies
```
- Копии, которые клонируют только друг друга, можно удалить (exit 0).
```
cargo test -p domovoi-dups --test pipeline
```
- Клоны и жёсткие ссылки оставляемого по-прежнему нельзя удалить, независимые копии — можно (exit 0).
```
cargo test -p domovoi-dups --test kept_copy
```
- Выбор оставляемого и копии в пакетах не изменились (exit 0).
```
cargo test -p domovoi-dups --test smoke_rules
```
- Удаление копий через лоток не изменилось (exit 0).
```
cargo clippy -p domovoi-dups -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
