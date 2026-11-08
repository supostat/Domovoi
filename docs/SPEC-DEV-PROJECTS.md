<!--
  SPEC-DEV-PROJECTS.md — дельта к docs/SPEC-DOMOVOI.md: найденные артефакты проектов разработки (target,
  node_modules, .next, .venv и др.) собираются по проектам в одну карточку «Проекты разработки», строка — проект со
  своим выбором. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 08.11.2026, выбран вариант A по рекомендации): проект — понятие движка правил,
  Recommendations.projects из того же ClaimLedger, экран, лоток, сценарий и Домовой видят одну группировку.
  Отвергнуты: B — проекты только в модели экрана (лоток снова раскладывает по правилам, монорепо по пакетам, список
  «проектных» правил в UI); C — проект как понятие индекса (новый FORMAT_VERSION и сброс кэша ради одной карточки).
  Вид выбран по макету до веера: одна карточка, строки — проекты, давние отмечены заранее.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/FIX-DIAGRAM-BACKDROP.md`
  (`0de4506` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. «Рекомендации» показывают одну карточку на правило: все `target/` всех проектов — «Каталоги target/
  (Rust)», все `node_modules` и `zig-cache` — «node_modules и zig-cache». Выбрать, в каком проекте чистить, нельзя:
  чекбокс один на карточку, пути склеены в строку, лоток снова собирает строки по правилу.
- Уже есть и не меняется: остальные карточки рекомендаций и их правила, гипотезы модели, удаление из лотка,
  denylist, итоги по уровням безопасности.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч (`crates/xtask/src/release.rs`).
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/rules`: новые правила в `rule_table.rs`; `project_card.rs` (новый) — `ProjectCard { root, items, blocked,
  reclaimable, last_work }`; `claim_ledger.rs` — `into_cards` раскладывает артефакты проектов в проекты;
  `recommendations.rs` — поле `projects`; `rule_set.rs` — признание артефакта проекта и корень проекта.
- `crates/app`: `project_row.rs` (новый) — `ProjectRow`; `card_model.rs` — `rows`; `recommendations_model.rs` —
  карточка проектов; `provenance.rs` — `Provenance::Projects`; `card_question.rs`; `recommendation_card.rs` — строки;
  `shell_actions.rs`, `shell_controller.rs` — выбор строк и групп, отправка в лоток; `shell_state.rs` — сохранение
  выбора; `strings.rs`; лоток — `crates/rules/src/tray.rs` (`TrayItem.project`) и `crates/app/src/tray_model.rs`
  (`RowGroup::Project`).
- Сценарий бандла: `end_to_end.rs`, `scenario_step.rs`, `crates/app/tests/wire_main_path.rs`.
- Документ и книга: `docs/DOMOVOI.md` (таблица правил и раздел рекомендаций), `design/domovoi-design-book.html`
  (образец 04).

# Conventions

- Один тип — один файл; имена без сокращений; комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`);
  строки интерфейса — в `strings.rs`.
- Подписи давности честные: индекс знает изменение файлов, а не открытие проекта — «без изменений больше месяца»,
  «менялись за последний месяц», давность строки — `format_age(last_work, now, DateSource::Modified)`.
- Выбор — только целым проектом; артефакты строки — подписи, не отдельные чекбоксы.
- Правило артефакта проекта — всегда «Безопасно»: карточка проектов несёт один уровень.
- Typecheck-критерий — `cargo clippy -p <крейт фазы> --all-targets -- -D warnings`; формат — `cargo fmt --all --
  --check` (заметка 7bbc2699).
- Вид карточки на снимке рисуется теми же функциями, что в живом окне, без подмен в пути снимка (заметка 0ab321e4).

# Knowledge

- **Сейчас.** `ClaimLedger::into_cards` ищет карточку по `card.rule == rule` (`crates/rules/src/claim_ledger.rs:175`)
  — одна `RuleCard` на правило; `CardItem { node, reclaimable }` хранит размер каждой папки, но `CardModel`
  (`crates/app/src/card_model.rs:15`) несёт только `paths` и общий `amount`; выбор — `toggle_card` по позиции
  карточки (`shell_controller.rs:890`), сохранение выбора — по `provenance` и `paths` (`shell_state.rs:278`);
  `send_cards_to_tray` кладёт цели выбранных карточек с `rule_id` (`shell_controller.rs:901`); лоток группирует по
  `RowGroup::Rule` (`crates/app/src/tray_model.rs:154`).
- **Шаблоны проектов.** Шаблон `**/<имя>|<маркер>` (`path_pattern.rs`) срабатывает на папку с именем, у родителя
  которой есть ребёнок-маркер (`rule_set.rs`, `matching_rule`); маркер проверяется и не запоминается. Такие шаблоны
  сейчас только у `cargo.target` (`**/target|Cargo.toml`) и `dev.deps` (`**/node_modules|package.json`,
  `**/zig-cache|build.zig`, `**/.zig-cache|build.zig`). Общий `target-dir` из `~/.cargo/config.toml` подключён
  `also_at` — путь без маркера, он вне проектов.
- **Артефакт проекта.** Пункт, взятый шаблоном `**/<имя>|<маркер>`, чей маркер лежит рядом с ним. Корень проекта —
  ближайший предок артефакта с ребёнком `.git` (папка или файл), если это не домашняя папка и не корень тома; иначе
  папка с маркером. Монорепо с `.git` — одна строка; без `.git` — строка на пакет. Защита от домашней папки нужна
  потому, что у части пользователей `~` — git-репозиторий dotfiles.
- **Последняя работа.** У каждой папки индекс хранит `newest_modified` — время самого свежего файла поддерева
  (`crates/index/src/index.rs:502-520`), его держит и путь дельты (`apply_delta.rs`). `last_work` проекта —
  наибольший `newest_modified` детей корня, кроме артефактов проекта и `.git`; ребёнок, внутри которого лежит
  артефакт, проходится вглубь по тому же правилу. Сборка в `target` и `git fetch` не делают давний проект свежим.
- **Новые правила (все «Безопасно»).**
    `dev.python`: `.venv`, `__pycache__`, `.mypy_cache`, `.pytest_cache` рядом с `pyproject.toml`, `requirements.txt` или `setup.py`
    `dev.web-build`: `.next` рядом с `next.config.js`, `next.config.mjs` или `next.config.ts`; `.nuxt` рядом с `nuxt.config.ts` или `nuxt.config.js`; `.turbo` рядом с `turbo.json`
    `dev.cocoapods`: `Pods` рядом с `Podfile`
    `dev.swiftpm`: `.build` рядом с `Package.swift`
- **Случаи проектов** (`crates/rules/tests/projects.rs`, правило → случай, который падает без него):
    корень без .git — папка маркера → `demo/target` рядом с `demo/Cargo.toml` даёт проект `demo`
    один корень — одна карточка проекта → `site/node_modules`, `site/.next`, `site/.turbo` дают один проект с тремя пунктами
    монорепо по .git → `mono/.git`, `mono/packages/a/node_modules`, `mono/packages/b/node_modules` дают один проект `mono`
    защита домашней папки → `.git` в домашней папке фикстуры не делает её корнем, проект — папка маркера
    общий target-dir вне проектов → путь `also_at` остаётся в карточке `cargo.target`
    last_work без артефактов и .git → свежий файл в `target` и в `.git` не меняет `last_work`, свежий исходник меняет
    denylist → артефакт под `chflags uchg` попадает в `blocked` проекта, а не в `items`
    итоги не меняются → `LevelTotals` с проектами равны итогам до группировки
    дельта → `package.json` рядом с `node_modules` создаёт проект, удаление `node_modules` убирает пункт, правка исходника обновляет `last_work`
    уровень → каждое правило с шаблоном `**/<имя>|<маркер>` — «Безопасно»
- **Макет вида.** Одна карточка «Проекты разработки»: заголовок, «N проектов», пояснение, бейдж «Безопасно», тег
  происхождения, затем две группы со своим чекбоксом — давние (отмечены заранее) и недавние (не отмечены); строка —
  чекбокс, имя корня, путь, артефакты подписями (повтор имени — «node_modules ×3»), давность, размер; порядок строк
  внутри группы — по размеру; сумма карточки — «отмечено из всего»; чекбокс карточки в смешанном состоянии, когда
  отмечена часть строк. Порог давности — константа 30 дней.
- **dev-clean (~/Projects/home/nm-remover).** Профили «папка + маркер у родителя» — то же правило, что здесь; у него
  строка — одна папка-артефакт без понятия проекта, давность — mtime самой папки артефакта (не говорит о работе в
  проекте), размер логический (жёсткие ссылки pnpm дважды) — эти места здесь сделаны иначе.
- **Домовой.** «Спросить Домового» на карточке проектов передаёт `CardFocus { rule: None, title, level, paths, bytes
  }` — пути артефактов по убыванию размера; шаг `ask_about_card` сценария сейчас ищет карточку по id правила
  (`end_to_end.rs:599`), в фазе wire он учится адресовать карточку проектов.
- **Уровень `.venv`.** «Безопасно» по аналогии с `node_modules`; пакеты, поставленные в окружение вручную мимо
  `requirements.txt` или `pyproject.toml`, после удаления придётся ставить заново — принято на ревью спеки.
- **Между фазами 1 и 2** приложение не показывает артефакты проектов: правила уже кладут их в `projects`, карточка
  появляется во второй фазе.
- **Ограничения.** `__pycache__` вложенных пакетов Python без маркера рядом не находится. Удаления частями внутри
  проекта нет. Вид строк, клики, смешанное состояние чекбокса и анимация — приёмка человеком на бандле.

# Gameplan

<!-- 3 фазы: правила, карточка, wire. Порядок фаз — порядок прогона. -->

### Phase 1: project-rules — находки правил собраны по проектам

- [ ] Новые правила по Knowledge «Новые правила» в `rule_table.rs`, заголовки в `RULE_TITLES`, строки таблицы правил в
  `docs/DOMOVOI.md`, фикстуры в `crates/rules/tests/rules_table.rs`.
- [ ] Признание артефакта проекта и корень проекта по Knowledge «Артефакт проекта»; `last_work` по Knowledge
  «Последняя работа».
- [ ] `ProjectCard` в `project_card.rs`; `into_cards` кладёт артефакты проектов в `Recommendations.projects` по
  убыванию reclaimable, остальное — в карточки правил; путь дельты отдаёт те же проекты.
- [ ] `crates/rules/tests/projects.rs` по Knowledge «Случаи проектов»; `smoke_index` и `recommendations_delta` — под
  проекты.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test projects
```
- Группировка, корень, монорепо, давность, denylist и дельта по случаям Knowledge (exit 0).
```
cargo test -p domovoi-rules --test rules_table
```
- Каждое правило, включая новые, берёт свою фикстуру (exit 0).
```
cargo test -p domovoi-rules --test doc_parity
```
- Таблица правил документа совпадает с движком по id и уровню (exit 0).
```
cargo test -p domovoi-rules --test recommendations_delta
```
- Дельта ведёт проекты так же, как полная сборка (exit 0).
```
cargo test -p domovoi-rules --test smoke_index
```
- Настоящий индекс даёт проекты и карточки правил (exit 0).
```timeout=600s
cargo test -p domovoi-rules --test recommendations_scale
```
- Пределы скорости сборки и дельты рекомендаций держатся (exit 0).
```
cargo clippy -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).

### Phase 2: projects-card — карточка «Проекты разработки» (потребляет фазу 1: Recommendations.projects)

- [ ] `ProjectRow` и `CardModel.rows`; `RecommendationsModel::build` строит карточку проектов по Knowledge «Макет
  вида»; `Provenance::Projects`; `card_focus` по Knowledge «Домовой».
- [ ] `RecommendationCard` рисует группы и строки; действия выбора строки и группы в `ShellActions` и контроллере;
  выбор строк переживает перестройку данных по пути корня (`shell_state.rs`).
- [ ] `send_cards_to_tray` кладёт цели выбранных строк с корнем проекта; `TrayItem.project`, `RowGroup::Project` —
  строка лотка на проект с именем и артефактами, «Снять отметку» снимает проект целиком.
- [ ] Фикстура `crates/app/tests/fixtures/shell_state.json` получает карточку проектов; `smoke_rules_dups`,
  `card_question`, `tray_unmark`, `cards_layout`, `truncation` — под строки.
- [ ] `crates/app/tests/projects_card.rs` на настоящем `Recommendations::build` по временному дереву: строки, группы,
  предвыбор давних, сохранение выбора, цели лотка с корнем.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test projects_card
```
- Карточка проектов собирается из настоящих правил фазы 1 (exit 0).
```
cargo test -p domovoi-app --test smoke_rules_dups
```
- Сессия превращает настоящие правила в модели экрана с карточкой проектов (exit 0).
```
cargo test -p domovoi-app --test tray_unmark
```
- Снятие строки проекта в лотке снимает все его артефакты (exit 0).
```
cargo test -p domovoi-app --test card_question
```
- Вопрос по карточке проектов несёт её пути и сумму (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Строки проектов не обрезают имя, путь и размер вслепую (exit 0).
```
cargo test -p domovoi-app --test cards_layout
```
- Высота карточек стабильна и укладывается в ширину (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Экраны ухода снимаются с карточкой проектов в обеих темах (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).

### Phase 3: wire — проекты на главном пути бандла (потребляет фазы 1-2)

- [ ] По скиллу wire-phase: `populate` добавляет Next.js-проект с `node_modules` и `.next` и монорепо с `.git` и двумя
  пакетами с `node_modules`; отчёт рекомендаций отдаёт строки карточки проектов (корень, артефакты, давность,
  выбор), проверки в `wire_main_path` в порядке шагов сценария.
- [ ] Шаг `ask_about_card` адресует карточку проектов вместо `cargo.target`.
- [ ] `docs/DOMOVOI.md` — абзац о карточке проектов в разделе рекомендаций; образец 04 в книге — карточка «Проекты
  разработки» вместо карточек `cargo.target` и `dev.deps`, подпись после скриншота headless Chrome.

**Done when (EXECUTABLE):**
```timeout=900s
cargo xtask bundle --profile adhoc
```
- Бандл собирается (exit 0).
```timeout=900s
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла показывает проекты, монорепо одной строкой и отвечает по карточке проектов (exit 0).
```
cargo xtask check-sources
```
- Документ и книга не расходятся с решениями (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
