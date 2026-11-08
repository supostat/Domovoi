<!--
  FIX-TRAY-REMAP.md — дельта к docs/SPEC-DOMOVOI.md: после изменения на диске строка проекта в листе «К удалению»
  становится строкой правила. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 08.11.2026, одна гипотеза, подтверждена пользователем с полным объёмом): Session::remap
  пересобирает лоток через Tray::add и теряет project, application, reason и список программ. Правка — пересборка,
  сохраняющая все поля пункта и программы, у которых остались пункты.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-DEV-PROJECTS.md`
  (`4a9a7db` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Отмеченный проект уходит в «К удалению» строкой проекта, но после первого же изменения на диске
  (FSEvents) строка становится строкой правила («Сборки SwiftPM»). По коду тем же путём строка программы распадается
  на отдельные папки.
- Уже есть и не меняется: вставка пунктов (`Tray::add`, `add_project_artifact`, `add_application`), группировка строк
  (`RowGroup`), снятие строки, смысл кнопки «Отметить к удалению» (добавляет к лотку, а не заменяет его).
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/rules/src/tray.rs`: `Tray::remapped(&self, index, resolve)` — новый лоток из пунктов, чей узел нашёлся,
  со всеми полями пункта; программы остаются, если у них остался пункт (то же правило, что в `Tray::remove`).
- `crates/app/src/session.rs`, `remap`: лоток пересобирается через `Tray::remapped`, а не `Tray::add`.
- `crates/app/tests/tray_remap.rs` (написан диагнозом): регрессия на уровне сессии.
- `crates/rules/tests/tray_remapped.rs` (новый): пересборка сохраняет проект, программу с причинами и правило;
  программа без оставшихся пунктов уходит из списка.

# Conventions

- Один способ пересобрать лоток: пункт не пересоздаётся по частям в вызывающем коде.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings`; формат —
  `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test tray_remap`:
    panicked at crates/app/tests/tray_remap.rs:97:5:
    assertion `left == right` failed: a change elsewhere on the volume keeps the project row in the tray
      left: ["Сборки SwiftPM"]
     right: ["condoit"]
- **Причина.** `Session::apply_listings` после каждого применённого изменения зовёт `remap` (`session.rs:208-209`);
  `remap` собирает `Tray::default()` и кладёт пункты через `tray.add(index, node, item.rule_id)`
  (`session.rs:497-503`), а `Tray::add` передаёт `None` вместо reason, application и project (`tray.rs:28`).
- **Программы.** Тем же путём теряются `application` и `reason` пунктов и список `applications` лотка — строка
  «Программа со всеми её файлами» распадается на папки. На уровне сессии не воспроизводилось; закрывается тестом лотка.
- **Проверка мутацией.** Одна мутация на прогон: вернуть в `remap` вызов `tray.add` — `tray_remap` краснеет.
- **Не входит.** Кэши в листе после нажатия кнопки: лоток копит отметки до удаления или снятия, «Отмена» только
  закрывает лист; замена отметок рекомендаций — отдельное решение.

# Gameplan

<!-- 1 фаза: пересборка лотка со всеми полями. -->

### Phase 1: tray-remap — лоток переживает изменения на диске целиком

- [ ] `Tray::remapped` по Stack; `Session::remap` пересобирает лоток через него.
- [ ] `crates/rules/tests/tray_remapped.rs`: проект, программа с причинами и правило переживают пересборку; программа,
  чьи пункты не нашлись, уходит из списка программ.
- [ ] Оставить `crates/app/tests/tray_remap.rs` регрессионным тестом; одна мутация по Knowledge.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test tray_remap
```
- Строка проекта остаётся строкой проекта после изменения на диске (exit 0).
```
cargo test -p domovoi-rules --test tray_remapped
```
- Пересборка лотка сохраняет проект, программу, причину и правило (exit 0).
```
cargo test -p domovoi-app --test tray_unmark
```
- Строки лотка и их снятие не изменились (exit 0).
```
cargo clippy -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
