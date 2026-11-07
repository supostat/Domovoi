<!--
  SPEC-WIRE-SKILL.md — дельта к docs/SPEC-DOMOVOI.md: у проекта появляется скилл .claude/skills/wire-phase —
  рецепт фазы wire (шаг --e2e, главный путь бандла, документ и книга, заметка «доказывает / не доказывает»). 1 фаза.

  ГРАНИЦА РЕШЕНИЯ (веер от 07.11.2026, выбран вариант A по рекомендации): скилл проекта в репозитории DOMOVOI —
  правится в тех же коммитах, что код, о котором он. Отвергнуты: B — плагин domovoi по образцу boss (второй
  репозиторий, рецепт отстаёт от кода, установка и версия ради одного скилла); C — общая часть в плагин mneme
  (теряются пути и команды DOMOVOI, правка публичного плагина).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-TRAY-MARKS.md`
  (`c866fb7`, `3077e9f`, `2ad61f2` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Фаза wire есть в каждой спеке с двумя и больше фазами кода (spec-tray-drag, spec-purge-space,
  spec-tray-marks), и каждый раз порядок собирается заново: где живёт шаг, где его место в сценарии, что пересобрать
  перед тестом, что делать с красным на образах, как проверить образец книги. Рецепта нет: `.claude/` в репозитории
  отсутствует.
- Уже есть и не меняется: шаги сценария `crates/app/src/scenario_step.rs` и `crates/app/src/end_to_end.rs`, тест
  `crates/app/tests/wire_main_path.rs`, сборка `cargo xtask bundle`, проверки `cargo xtask check-*`.
- Версия: фазы повышения версии нет — скилл не входит в продукт, `cargo xtask release` сам инкрементирует патч.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `.claude/skills/wire-phase/SKILL.md` (новый): frontmatter `name: wire-phase` и `description` с поводами вызова;
  тело — рецепт по Knowledge. `.gitignore` исключает только `.claude/settings.local.json` — файл попадает в git.

# Conventions

- Язык тела скилла и `description` — английский, как у скиллов boss; вывод пользователю — русский.
- Скилл называет файлы, функции и команды, без номеров строк — номера уходят при каждой правке.
- В начале строка: код расходится с файлом — прав код, файл правится в том же коммите.
- Скилл не повторяет `docs/DOMOVOI.md` и спеки: только порядок работы фазы wire.

# Knowledge

- **Шаг сценария.** `ScenarioStep` (`scenario_step.rs`) — `serde(tag = "step", rename_all = "snake_case",
  deny_unknown_fields)`; новый шаг — вариант там, имя в `step_name` и ветка в `EndToEnd::perform` (`end_to_end.rs`).
  Метод шага идёт через функции приложения, а не копирует их: бросок — `Session::judge_drop` и `drop_into_tray`,
  уровень — `ShellData::level_rows` и `MainScreen::diagram_state`. `live_data` повторяет `ShellState::rebuild_data`:
  шаг, добавленный в перестройку данных приложения (как `ShellData::mark_tray`), добавляется и в `live_data`.
  Отчёт — JSON с полями snake_case. `Result` только у шага, который может упасть: иначе clippy
  `unnecessary_wraps` (заметка 8959d650).
- **Сценарий и проверки.** Фикстуры — константы в начале `wire_main_path.rs` и `populate`. Место шага в
  `scenario()` — где состояние тома такое, какое шаг ждёт, и откуда он не сдвигает то, что проверяют следующие шаги:
  `mark_from_list` и `unmark_tray_row` стоят после `delete_recommendations` `check_yourself` (список пуст до и после),
  `purge_space` — после `tmutil_commands`, за ним второй `hidden_space`. Проверки — функция `assert_<фича>` через
  `step` и `steps`, вызов в `the_bundled_application_walks_the_main_path_on_real_modules` после прежних.
- **Прогон.** Тест запускает `target/bundle/DOMOVOI.app/Contents/MacOS/domovoi --e2e <сценарий> --report <отчёт>`
  (`command_line.rs`); бандл пересобирается перед каждым прогоном: `cargo xtask bundle --profile adhoc`, затем
  `cargo test -p domovoi-app --test wire_main_path`.
- **Красное на образах.** Сначала строка паники. Своя `assert_<фича>` — дефект фазы. Паника в `ImageVolume::create`
  («the image attaches», параллельные тесты с образами, заметка 46dd48ef) или в чужой проверке (`purge_space`,
  `freed 32768`, заметка 08193ac5) — повтор на том же бандле до четырёх раз с цитатой вывода; зелёные повторы —
  заметка-решение с цитатой красного и повторов и отдельный `/mneme:fix`, внутри фазы не чинить; красное держится —
  разбирать как дефект.
- **Документ и книга.** Абзац в `docs/DOMOVOI.md` рядом с разделом фичи. Образец в `design/domovoi-design-book.html`
  проверяется скриншотом headless Chrome (`--headless=new --disable-gpu --hide-scrollbars --user-data-dir=<временная
  папка> --window-size=<ш>,<в> --screenshot=<png> file://<путь книги>`); подпись пишется после чтения снимка и
  называет то, что на нём видно — в run cdec2202 подпись назвала Music, а второй строкой образца был Containers.
  После правок — `cargo xtask check-sources`.
- **Перед гейтом.** `cargo test -p domovoi-app` целиком, `cargo clippy -p domovoi-app --all-targets -- -D warnings`,
  `cargo xtask check-comments`, `cargo xtask check-tokens`, `cargo xtask check-sources`.
- **Заметка фазы.** Decision: шаг и поля отчёта, место в сценарии, проверки, абзац документа, образец книги.
  Rationale: «Главный путь доказывает цепочку на настоящих модулях бандла: A → B → C», «Не доказывает (приёмка
  человеком на бандле): …», «Тест читает target/bundle — бандл пересобирается перед ним». Образцы: заметки 85cfb548,
  21727376, 856e651f.
- **Предел проверки.** Что рецепт верен на практике, команда не проверяет: скрипта, сверяющего пути скилла с
  репозиторием, нет (цена варианта A). Это сверка чтением на границе фазы.

# Gameplan

<!-- 1 фаза: прозаический скилл, кода нет — typecheck-критерия и фазы wire нет. -->

### Phase 1: wire-skill — скилл проекта wire-phase

- [ ] Написать `.claude/skills/wire-phase/SKILL.md` по Stack, Conventions и Knowledge: frontmatter, образцы,
  шаг сценария, сценарий и проверки, прогон, красное на образах, документ и книга, перед гейтом, заметка фазы.
- [ ] `description` называет поводы: фаза с id wire в DOMOVOI, новый шаг на главном пути бандла, красный
  `wire_main_path`.

**Done when (EXECUTABLE):**
```
grep -q name:.wire-phase .claude/skills/wire-phase/SKILL.md
```
- Скилл есть и назван wire-phase (exit 0).
```
grep -q scenario_step.rs .claude/skills/wire-phase/SKILL.md
```
- Раздел шага сценария называет файл варианта (exit 0).
```
grep -q adhoc .claude/skills/wire-phase/SKILL.md
```
- Раздел прогона называет пересборку бандла (exit 0).
```
grep -q ImageVolume::create .claude/skills/wire-phase/SKILL.md
```
- Раздел красного на образах называет место флейка (exit 0).
```
grep -q headless .claude/skills/wire-phase/SKILL.md
```
- Раздел книги называет скриншот (exit 0).
```
grep -q unnecessary_wraps .claude/skills/wire-phase/SKILL.md
```
- Урок clippy о шаге без отказа в рецепте (exit 0).

**Done when (AGENT-JUDGED):** каждый файл, функция и команда скилла существуют в репозитории, а рецепт совпадает с тремя прошлыми фазами wire (85cfb548, 21727376, 856e651f) — соответствие текста практике командой не проверяется, скрипта сверки путей в варианте A нет.
