<!--
  FIX-REBUILD-WITHOUT-SESSION.md — дельта к docs/SPEC-DOMOVOI.md: пока задача держит сессию, перестройка экранов
  стирает рекомендации, дубликаты, лоток и дерево диаграммы, а отметки пользователя теряются. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, две гипотезы, выбрана первая): ShellState::rebuild_data забирает прежние данные
  экранов и без сессии строит пустые; правка — без сессии сохранять данные, зависящие от индекса.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/FIX-TRAY-REMAP.md`,
  после неё правки аудита вкладок без спек (`60bb346` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Сессию на время работы забирают задачи: поиск дубликатов, ответ Домового, скан, тяжёлая дельта. Если в
  это время экраны перестраиваются (конец «Освободить…» в контекстном меню «Скрытое»), «Рекомендации» пустеют, а
  когда сессия возвращается, отметки пользователя уже потеряны.
- Уже есть и не меняется: задачи, забирающие сессию; события моделей, которые ждут сессию (`pending_model_events`);
  перенос отметок `keep_selection` при перестройке с сессией.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/app/src/shell_state.rs`, `rebuild_data`: без сессии данные, зависящие от индекса (дерево диаграммы,
  рекомендации, дубликаты, лоток, отметки дерева, `care_ready` и счётчики индекса), остаются прежними; обновляется то,
  что от индекса не зависит: обзор диска, доступ, статус скана, модели Домового. До первого индекса всё строится как
  сейчас.
- `crates/app/src/shell_state.rs`, модуль `tests` (написан диагнозом): регрессия на уровне `ShellState`.

# Conventions

- Один способ перестроить экраны: правило «без сессии данные индекса не трогаются» живёт в `ShellState`, а не в местах
  вызова `refresh`.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-app --all-targets -- -D warnings`; формат — `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --lib shell_state::tests`:
    panicked at crates/app/src/shell_state.rs:394:9:
    assertion `left == right` failed: a rebuild without the session keeps what the screens showed
      left: RecommendationsModel { safe: 0, recoverable: 0, check_yourself: 0, cards: [] }
     right: RecommendationsModel { safe: 1048576, … cards: [CardModel { title: "Проекты разработки", … selected: true … }] }
- **Причина.** `rebuild_data` забирает прежние данные (`self.data.take()`, `shell_state.rs:174`), а `care_models` без
  сессии возвращает `CareModels::default()` (`shell_state.rs:260-267`); дерево строится с `index = None`. Когда сессия
  возвращается, `keep_selection` сравнивает с пустой моделью.
- **Где срабатывает.** Конец «Освободить…»: `purge_tick` зовёт `refresh()` без проверки сессии
  (`shell_controller.rs:1433`), а `PurgeSpace` не входит в `ShellAction::needs_session`. Память проекта (заметка
  9a57fd76) уже предупреждала, что refresh без сессии обнулит данные экранов.
- **Отвергнуто.** Отложить refresh в `purge_tick` до возврата сессии: правило осталось бы на совести каждого места
  вызова, а контроллер из тестов недоступен.
- **Компромисс.** Пока задача держит сессию, строка purgeable на диаграмме показывает прежнее значение; свежее —
  после возврата сессии, когда задача сама зовёт refresh.
- **Репро-тест.** Модуль `#[cfg(test)]` в конце `shell_state.rs` (ShellState — pub(crate), из crates/app/tests
  недоступен), как в `rules/src/path_pattern.rs` и `scanner/src/node_state.rs`; место согласовано с пользователем.
- **Проверка мутацией.** Одна мутация на прогон: вернуть построение пустых моделей без сессии — тест краснеет.
- **Не входит.** Остальные находки аудита вкладок: строки проектов с 0 Б, остатки после переустановки, общие блоки
  клонов, ошибка поиска дубликатов, Quick Look, замена клоном на главном потоке.

# Gameplan

<!-- 1 фаза: экраны переживают задачу, которая держит сессию. -->

### Phase 1: rebuild-without-session — экраны переживают задачу, которая держит сессию

- [ ] `ShellState::rebuild_data` без сессии сохраняет данные экранов, зависящие от индекса, и обновляет остальное по Stack.
- [ ] Оставить `shell_state::tests` регрессионным тестом; одна мутация по Knowledge.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --lib shell_state::tests
```
- Перестройка без сессии сохраняет рекомендации и отметки (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
```
cargo xtask check-comments
```
- Комментарии не ссылаются на спеку (exit 0).
