<!--
  SPEC-CARD-QUESTION.md — дельта к docs/SPEC-DOMOVOI.md: кнопка «Спросить Домового» на каждой карточке экрана
  «Рекомендации» задаёт ассистенту вопрос об этой карточке. 3 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 28.10.2026, выбран вариант A по рекомендации): карточка едет агенту полем
  FocusContext.card в сводку вопроса, агент сам вызывает rule_explain и inspect_node, первый шаг локальной модели —
  обязательный вызов инструмента. Отвергнуты: B — досье от приложения (гарантированные данные на маленькой модели, но
  сводка растёт на килобайты, новое понятие «шаг досье», для Claude избыточно); C — режим разбора с вердиктом модели
  (противоречит принципу «модель объясняет, решают правила», большой объём в грамматике, MCP, эвале и чате).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-TRAY-NAME.md`
  (влита в `main`, `8cbe602`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. На карточке рекомендаций есть заголовок, пути, объяснение правила и размер, но нет способа спросить, что
  именно лежит по этим путям в доме пользователя и стоит ли это удалять. Сейчас для этого нужно перейти в «Домовой» и
  описать карточку словами, а пути в тексте вопроса — канал для враждебных имён.
- Уже есть и не меняется: путь вопроса `Assistant::begin → AskJob → Session::ask` для локальной модели и Claude Code;
  `Question { text, history, focus }`; сводка `ToolRunner::overview` с `focus`, `selected`, `last_paths` под
  `untrusted_json`; инструменты `rule_explain`, `inspect_node`, `query_index`, `summarize`; действия только из узлов
  инструментов (`AgentAnswer::assembled`); `Agent::calling_a_tool_first`; кнопка «Отметить к удалению» под ответом;
  экраны «Домовой» в состояниях Off / NoModel с переходом к «Модели»; гейт `ShellAction::needs_session`.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/agent/src/card_focus.rs` (новый): `pub struct CardFocus { rule: Option<String>, title: String, level:
  SafetyLevel, paths: Vec<String>, bytes: u64 }`; `FocusContext` получает `card: Option<CardFocus>` (`none()` — `None`).
- `crates/agent/src/tool_runner.rs`: `overview()` при `focus.card` кладёт `card`: `rule`, `title`, `level`
  (`level_name`), `paths` — первые `CARD_PATHS = 10`, `paths_total`, `bytes`.
- `crates/agent/src/instructions.rs`: строка «Если в сводке есть card — пользователь спрашивает об этой карточке
  рекомендаций: объясни правило через rule_explain, посмотри крупнейшие её пути через inspect_node и скажи, что там
  лежит, можно ли удалять и что останется занято».
- `crates/agent/src/agent.rs`: первый шаг под `tool_calls_only`, если `first_step_calls_tool` или `context.focus.card`
  задана.
- `crates/app/src/strings.rs`: `ASK_DOMOVOI` = «Спросить Домового»; `CARD_QUESTION` = «Разбери эту карточку: что лежит
  по её путям, почему она в рекомендациях, можно ли удалять и что останется занято.»
- `crates/app/src/card_question.rs` (новый): `card_focus(card: &CardModel) -> CardFocus` — правило из
  `Provenance::Rule`, у гипотезы `None`; уровень через `From<SafetyBadge> for SafetyLevel` (`safety_badge.rs`).
- `crates/app/src/shell_action.rs`: `AskAboutCard(usize)`, в `needs_session`; `shell_actions.rs`: селектор
  `askAboutCard:` с позицией в теге.
- `crates/app/src/recommendation_card.rs`: кнопка `ASK_DOMOVOI` под содержимым карточки, высота карточки учитывает её;
  `cards_model.rs` / `cards_source.rs`: `ask_enabled: Cell<bool>`, `show(cards, ask_enabled)` перестраивает карточки
  при смене карточек или доступности; `recommendations_screen.rs` передаёт
  `!matches!(availability, Loading | Answering)`.
- `crates/app/src/shell_controller/assistant_actions.rs`: общий `ask(question, focus)` для черновика и карточки;
  `AskAboutCard(position)` открывает раздел «Домовой» и, если `accepts_questions()`, задаёт `CARD_QUESTION` с фокусом
  `{ folder, selected, card: Some(card_focus(card)) }`; при Off / NoModel только открывает раздел.
- `crates/app/src/scenario_step.rs`, `end_to_end.rs`: шаг `ask_about_card { rule }` — карточка правила из текущих
  рекомендаций, вопрос тем же путём, что `ask_assistant`; отчёт `question`, `card_rule`, `tools`,
  `answer_characters`, `history_turns`.
- `docs/DOMOVOI.md` 4.6 и 4.8 (экран 4); `design/domovoi-design-book.html` экран 04 — кнопка на карточке и подпись.

# Conventions

- В тексте вопроса нет ни путей, ни заголовка карточки: заголовок гипотезы — имя папки, то есть данные. Всё о карточке
  едет в сводке под `untrusted_json`.
- Модель объясняет, решают правила: уровень карточки не меняется ответом, действия — только из узлов инструментов.
- Голос книги: без восклицаний, без «AI».
- Один путь вопроса для черновика и карточки — без второго `begin`.

# Knowledge

- Сводка `overview` общая для локального агента и Claude Code (MCP поверх того же `ToolRunner`), поэтому `card` видят
  оба. Обязательный первый вызов инструмента действует только в локальном `Agent`; Claude ведёт инструкция.
- `Availability::Loading` и `Answering` — вопрос уже в работе (`job.is_some()`); только тогда кнопка недоступна. При
  Off / NoModel кнопка ведёт в «Домовой», где экран объясняет, что включить или скачать.
- Карточки живут в `NSCollectionView`, высота считается тем же `RecommendationCard::build` — кнопка меняет высоту, это
  проверяют `snapshot_care` и `truncation`.
- Позиция в теге кнопки — как у `toggleCard:`: карточки пересобираются на дельтах, но между построением и нажатием
  набор тот же, что и для чекбокса.
- Пути карточки бывают десятками (`node_modules` по проектам) — в сводку идут первые 10 и `paths_total`.
- Сквозной сценарий: `download_model` переезжает перед `delete_recommendations`, а `ask_about_card { rule:
  "cargo.target" }` идёт сразу за ним — после удаления карточки `cargo.target` уже нет. Вопрос по карточке становится
  первой репликой разговора, поэтому у двух `ask_assistant` `history_turns` 1 и 2.
- Предел проверки: качество разбора на настоящей модели — ручная приёмка (tiny-GGUF отвечает случайно); граница фазы
  wire — точка приёмки человеком: кнопка на карточке, переход в «Домовой», ответ с шагами и «Отметить к удалению».

# Gameplan

### Phase 1: agent-card — карточка в сводке и первый вызов инструмента

- [ ] `agent/src/card_focus.rs`, `FocusContext.card`, `overview()["card"]`, строка `INSTRUCTIONS`, первый шаг
  `tool_calls_only` при карточке, экспорт `CardFocus`.
- [ ] `agent/tests/card_focus.rs` (новый) на `FakeBackend`: карточка в сводке первого промпта, в тексте вопроса нет её
  путей, враждебное имя пути остаётся внутри `untrusted_json`, первый запрос — под схемой только инструментов, больше
  10 путей — `paths_total`; без карточки поведение прежнее.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent --test card_focus
```
- Карточка в сводке, путей нет в вопросе, первый шаг — инструмент (exit 0).
```
cargo test -p domovoi-agent --test injection
```
- Враждебные имена не меняют поведения (exit 0).
```
cargo clippy -p domovoi-agent --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: card-button — кнопка на карточке и вопрос из приложения

<!-- deps: [agent-card] — потребление: CardFocus и FocusContext.card из фазы 1. -->

- [ ] Строки, `card_question.rs`, `From<SafetyBadge> for SafetyLevel`, `ShellAction::AskAboutCard`, селектор, кнопка в
  `RecommendationCard`, `ask_enabled` в карточках, общий `ask(question, focus)` в контроллере.
- [ ] `app/tests/card_question.rs` (новый): карточка правила и карточка гипотезы → `card_focus` (правило, уровень,
  пути); настоящий `Agent::ask` на `FakeBackend` с этим фокусом получает карточку в сводке.
- [ ] `app/tests/voice.rs`: `ASK_DOMOVOI`, `CARD_QUESTION` без «!» и без путей.
- [ ] `docs/DOMOVOI.md` 4.6, 4.8; книга экран 04.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test card_question
```
- Фокус карточки доходит до настоящего агента (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Строки кнопки и вопроса (exit 0).
```
cargo test -p domovoi-app --test truncation
```
- Карточка с кнопкой не обрезает текст (exit 0).
```
cargo test -p domovoi-app --test snapshot_care
```
- Экран рекомендаций держит размеры (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: wire — вопрос по карточке на главном пути бандла

<!-- deps: [agent-card, card-button] — потребление: сценарий гонит настоящий AskAboutCard через Session::ask. -->

- [ ] `scenario_step.rs`, `end_to_end.rs`: шаг `ask_about_card { rule }`.
- [ ] `app/tests/wire_main_path.rs`: `download_model` перед `delete_recommendations`, за ним `ask_about_card`
  (`cargo.target`); проверки: вопрос — `CARD_QUESTION`, правило карточки `cargo.target`, есть шаг инструмента и
  непустой ответ; `history_turns` у двух `ask_assistant` — 1 и 2.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла с вопросом по карточке (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo xtask check-tokens
```
- Цвета только в теме (exit 0).
```
cargo clippy -p domovoi-agent -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
