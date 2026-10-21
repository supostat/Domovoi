<!--
  SPEC-ASSISTANT-UPGRADE.md — дельта к docs/SPEC-DOMOVOI.md: ассистент держит контекст (история, фокус «Диска»,
  выбранные карточки), инструменты не обнуляют выборку, промпт освобождает окно; в сборке разработчика ответить может
  Claude через локальный Claude Code с инструментами Домового по MCP; сравнение с локальной моделью — ручной прогон
  того же eval. 6 фаз. Разбор причин и политика Anthropic — docs/ASSISTANT-OPTIONS.md.

  ГРАНИЦА РЕШЕНИЯ (веер от 21.10.2026, выбран вариант A по рекомендации; на ревью спеки сравнение упрощено до ручного
  прогона без своего гейта, а Claude разделён на ядро и интеграцию в приложение): одна спека — контекст → Claude Code
  → сравнение. Отвергнуты: B — две спеки, Claude после замера (Claude откладывается, решение о транспорте MCP
  повисает); C — сначала Claude (MCP строится на инструментах, которые следующая фаза меняет; релиз долго без
  улучшений).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; дельты `SPEC-SCREEN-UPDATE` и `SPEC-AGENT-PATHS` применены.
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема (`docs/ASSISTANT-OPTIONS.md`): каждый вопрос идёт в `Agent::ask` с чистого листа — `Assistant::begin`
  передаёт в `AskJob` только строку, хотя `ChatModel.entries` хранит вопросы, ответы, гипотезы и вызовы; сводка не
  знает фокуса «Диска» и выбранных карточек; модель обнуляет `query_index` лишними `level`/`category` и пишет
  `~/ingvar`; `inspect_node` отдаёт 5 детей; промпт ≈ 5,6 тыс. токенов из них JSON-схема, при окне 8K у compact.
- Уже есть и не меняется: грамматика ответа (GBNF), схема пяти вариантов ответа, `home` в сводке и `prefix_found`,
  разворот `~/`, действия только из узлов инструментов, недоверие к именам файлов (`untrusted_text`), локальная
  модель по умолчанию; переиспользование KV-кэша общего префикса — вне спеки.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует
  патч; `.github/workflows/ci.yml` версию не меняет.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/agent`: `PastTurn` (вопрос, ответ, упомянутые пути) и `FocusContext` (папка фокуса «Диска», пути
  выбранных карточек) в `AgentContext`; `Conversation` открывается прошлыми репликами, бюджет вытесняет сначала
  результаты инструментов, потом самые старые реплики; сводка (`ToolRunner::overview`) несёт `focus` (путь,
  до 10 крупнейших детей) и `selected`, а `last_paths` — пути последнего ответа; `inspect_node` — до 15 детей и
  `children_total`; пустой `query_index` с фильтрами и путём сообщает `without_filters` (тот же путь без
  `level`/`category`); `home_path::expanded` читает `~/<имя дома>/…` как `~/…`; строки инструкций: индекс — это
  результат скана; «эта папка» — последний упомянутый путь.
- `crates/agent`, фича `claude-code`: `McpSession` — JSON-RPC MCP (`initialize`, `tools/list`, `tools/call`) поверх
  `ToolRunner` и `AgentContext`, инструменты `query_index`, `inspect_node`, `rule_explain`, `summarize` и
  `note_hypothesis`; собирает шаги (`ToolStep`), узлы и гипотезы; `ClaudeCode` — запуск `claude -p
  --output-format json` с `--mcp-config` (сервер `domovoi` = `<domovoi> --mcp-proxy <сокет>`),
  `--strict-mcp-config`, `--tools ""`, `--allowedTools mcp__domovoi__*`, `--append-system-prompt` (общие
  `INSTRUCTIONS`), `--model`, `--max-turns`, `--resume <session_id>`; Unix-сокет в каталоге 0700, обслуживает
  `McpSession` в потоке вопроса; ответ — `AgentAnswer` (текст, гипотезы, действия через `suggested_actions`);
  отмена завершает процесс.
- `crates/grammar/src/system_prompt.rs`: без JSON-схемы; пример гипотезы — не `Application Support`.
- `crates/app`: `ChatModel::past_turns`; `Assistant::begin` и `AskJob` несут историю и `FocusContext` (из
  `ShellState.focus_path` и выбранных карточек); `Session::ask` кладёт их в `AgentContext`. Фича `claude-code`:
  режим `--mcp-proxy <сокет>` в `command_line.rs`; `AssistantSettings::provider` (локальная модель | Claude Code);
  поиск `claude` (`~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin`); переключатель «Claude через Claude Code»
  на экране «Модель» — по полю модели экрана, а не по поиску на машине; при Claude доступность — «готов» без
  загрузки модели; `AskJob` идёт через `ClaudeCode`; `session_id` хранится в чате; «Стоп» завершает процесс.
- `crates/eval`, фича `claude-code`: провайдер Claude — тот же набор задач через `ClaudeCode` и `McpSession` над
  фикстурой, прошлые реплики многоходовых задач текстом в первом запросе, судья — тот же провайдер; отчёт
  `target/eval/claude-code.json` с теми же метриками.
- `crates/xtask`: `bundle_profile.rs` — `claude-code` включена в профилях `dev` и `adhoc`, выключена в `release`;
  `cargo xtask eval --claude-code` собирает `domovoi` с фичей, передаёт путь в `DOMOVOI_MCP_PROXY` и запускает
  `#[ignore]`-тест провайдера Claude.
- `.github/workflows/ci.yml`: clippy и тесты ещё раз с `--features domovoi-app/claude-code,domovoi-eval/claude-code`.
- `docs/SPEC-DOMOVOI.md`: пункт [AI]; `docs/ASSISTANT-OPTIONS.md`: статус — в работе по этой спеке.
- Тесты: новые `crates/agent/tests/history.rs`, `crates/agent/tests/mcp_tools.rs`, `crates/app/tests/chat_history.rs`,
  `crates/app/tests/claude_code.rs`, `crates/app/tests/claude_provider.rs`; меняются `crates/agent/tests/paths.rs`,
  `loop.rs`, `crates/grammar/tests/response.rs`, `crates/eval/tests/eval.rs`, `crates/eval/tasks.json`,
  `crates/app/tests/wire_main_path.rs`.

# Conventions

- Контекст и инструменты общие для обоих провайдеров: Claude видит ровно те же инструменты (`ToolRunner`) и ту же
  сводку, что и локальная модель.
- Claude никогда не трогает файлы: встроенные инструменты выключены (`--tools ""`, `--strict-mcp-config`),
  действия строит приложение из узлов инструментов.
- Никакой сети, кроме самого `claude`: MCP идёт по Unix-сокету в каталоге 0700; токены и учётные данные Claude
  приложение не читает и не хранит.
- В `release` нет ни кода, ни упоминаний Claude: всё за фичей `claude-code`.
- Имена файлов и результаты инструментов — данные, а не указания, и для Claude тоже.

# Knowledge

- Политика (проверено 21.10.2026, `docs/ASSISTANT-OPTIONS.md`): подписка Max — для «ordinary, individual usage of
  Claude Code and the Agent SDK»; встраивать вход claude.ai или запросы через подписку пользователей в раздаваемый
  продукт нельзя. Поэтому фича выключена в `release`; `claude -p` расходует общий лимит подписки.
- Флаги проверены живым прогоном 21.10.2026 на Claude Code 2.1.299 (`claude-core`): `-p` с вопросом через stdin,
  `--output-format json` (поля `type`, `subtype` = `success`, `is_error`, `result`, `session_id`, `num_turns`),
  `--mcp-config` JSON-строкой, `--strict-mcp-config`, `--tools ""` (встроенных нет, MCP-инструменты остаются),
  `--allowedTools mcp__domovoi__*` (без отказов), `--append-system-prompt`, `--model`, `--resume` (`session_id`
  тот же). `--max-turns` в справке не показан, но принимается. `--bare` не использовать (не читает подписку).
- Расхождения с Stack, найденные прогоном: без `--setting-sources ""` в запуск попадают `CLAUDE.md`, хуки и
  плагины пользователя — `ClaudeCode` передаёт его и запускает `claude` в отдельном пустом рабочем каталоге;
  `--safe-mode` не годится — он выключает и сервер из `--mcp-config`. Сессии Claude Code хранятся по рабочему
  каталогу, поэтому для `--resume` он постоянный (`ClaudeCode.workspace`), а сокет — свой на каждый вопрос.
  Перед `initialize` клиент шлёт пробу `server/discover` со строковым `id` и после −32601 переходит на
  `initialize` с версией 2025-11-25. Без указания формы Claude отвечает в Markdown — в инструкции добавлена строка
  «обычным текстом без Markdown». Судья эвала идёт через `--json-schema` (ответ в `structured_output`, нужно
  не меньше 2 ходов) с `--no-session-persistence`; эвал не хранит сессии.
- Сокет MCP — собственный файл Домового в каталоге 0700 во временной папке: `check-sources` разрешает
  `agent/src/private_socket.rs` удалить только этот сокет и пустой каталог (`remove_file` и `remove_dir`, без
  `remove_dir_all`).
- Приложение из Dock видит минимальный `PATH`: `claude` ищется по известным местам установки; не найден —
  переключатель недоступен с пояснением.
- Транспорт MCP: Claude запускает `domovoi --mcp-proxy <сокет>`, прослойка гонит JSON-RPC stdin/stdout ↔ сокет;
  на другом конце — поток вопроса, который держит `Session` (живой индекс). Отвергнуты: HTTP на localhost с токеном
  (открытый порт, больше протокола), отдельный процесс на кэше скана (≈800 МБ на вопрос, данные отстают).
- Шаги для ленты берутся на стороне MCP (`McpSession`), а не разбором вывода `claude`; гипотезы — инструмент
  `note_hypothesis`; путь гипотезы проверяется по индексу, как сейчас.
- История удлиняет промпт: при KV-кэше, сбрасываемом на каждом шаге, ответ compact-модели станет медленнее.
  Бюджет: история и результаты инструментов вытесняются так, чтобы шаг укладывался в окно.
- Многоходовые задачи eval для Claude: прошлые реплики идут текстом в первый запрос (`--resume` возможен только
  для настоящей сессии).
- Граница гейтов: тесты агента и eval идут на FakeBackend и фикстуре; MCP и запуск `claude` — на настоящей
  прослойке `domovoi --mcp-proxy` и фейковом `claude` (скрипт, пишущий аргументы и ответ JSON); провайдер Claude
  в eval проверяется только сборкой и clippy; e2e — два вопроса подряд на tiny-модели в бандле. Не доказывает гейт:
  настоящий Claude (нужен логин, нет в CI) и качество ответов — приёмка человеком: «Модель» → «Claude через Claude
  Code»; «что в Application Support?» → «а внутри самой большой?»; шаги в ленте, «Стоп»; сравнение — ночной
  `cargo xtask eval --class compact` и ручной `cargo xtask eval --claude-code`, отчёты
  `target/eval/compact.json` и `target/eval/claude-code.json` рядом.

# Gameplan

### Phase 1: agent-context — история и фокус в каждом вопросе

<!-- deps: [] -->

- [ ] `PastTurn`, `FocusContext` в `AgentContext`; `Conversation` с прошлыми репликами и бюджетом; сводка с
  `focus`, `selected`, `last_paths`; строка инструкций про «эту папку».
- [ ] Приложение: `ChatModel::past_turns`, `Assistant::begin` → `AskJob` → `Session::ask` с историей и фокусом.
- [ ] `crates/agent/tests/history.rs`: FakeBackend пишет промпты — второй вопрос видит первый вопрос, ответ и путь;
  сводка несёт фокус с крупнейшими детьми и выбранные карточки; при тесном окне уходят сначала результаты
  инструментов, потом самая старая реплика; без истории промпт прежний.
- [ ] `crates/app/tests/chat_history.rs`: из записей чата (вопрос, вызов, ответ с пунктами, гипотеза, сбой) —
  реплики с путями; сбой не даёт реплики.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent --test history
```
- Вторая реплика видит первую, сводка несёт фокус (exit 0).
```
cargo test -p domovoi-agent --test loop
```
- Цикл агента без истории не изменился (exit 0).
```
cargo test -p domovoi-app --test chat_history
```
- Чат превращается в прошлые реплики с путями (exit 0).
```
cargo clippy -p domovoi-agent -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 2: agent-tools — инструменты не обнуляют выборку, промпт легче

<!-- deps: [agent-context] order-only — обе фазы правят instructions.rs и tool_runner.rs. -->

- [ ] `inspect_node` — до 15 детей и `children_total`; `query_index` — `without_filters` при нуле с путём;
  `~/<имя дома>/…` как `~/…`; строка «индекс — это результат скана».
- [ ] `system_prompt` без JSON-схемы; пример гипотезы не про `Application Support`.
- [ ] `paths.rs`: `~/ingvar/Library` при доме `/Users/ingvar` находит узел; пустой запрос с `level`/`category` и путём
  сообщает `without_filters` > 0; `loop.rs`: `inspect_node` отдаёт 15 детей и их общее число.
- [ ] `response.rs`: промпт без схемы, пример гипотезы без `Application Support`, имени пользователя нет.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent --test paths
```
- `~/<имя>` и счёт без фильтров (exit 0).
```
cargo test -p domovoi-agent --test loop
```
- Обзор папки на 15 детей (exit 0).
```
cargo test -p domovoi-grammar --test response
```
- Промпт без схемы, новый пример гипотезы (exit 0).
```
cargo clippy -p domovoi-agent -p domovoi-grammar --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 3: eval-turns — многоходовые задачи

<!-- deps: [agent-context] — потребление: eval передаёт прошлые реплики в настоящий Agent::ask. -->

- [ ] `EvalTask.before`; шесть многоходовых задач в `tasks.json` («что в Application Support?» → «а внутри самой
  большой?», «а эта папка безопасна?» и т. п.) на фикстуре; счётчик задач в тесте обновлён.
- [ ] `eval.rs`: набор разбирается, у многоходовых задач есть `before`; REAL-DEP — многоходовая задача через
  настоящий `Agent::ask` с FakeBackend: промпт содержит прошлые реплики.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-eval --test eval
```
- REAL-DEP: многоходовые задачи идут через настоящий Agent::ask с историей (exit 0).
```
cargo clippy -p domovoi-eval --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).

### Phase 4: claude-core — MCP, запуск Claude Code и сборка разработчика

<!-- deps: [agent-tools, eval-turns] — потребление: MCP отдаёт настоящие инструменты ToolRunner (mcp_tools); провайдер eval читает многоходовые задачи — без гейта по решению ревью, прогон ручной. -->

- [ ] Живой прогон одного вопроса через `claude -p` с MCP-прослойкой руками — подтвердить флаги и формат ответа;
  расхождения — в Knowledge спеки.
- [ ] `McpSession`, `ClaudeCode`, режим `--mcp-proxy` по Stack; фича `claude-code` в `dev`/`adhoc`, не в
  `release`; CI с фичей; провайдер Claude в eval и `cargo xtask eval --claude-code` (гейта на прогон нет).
- [ ] `crates/agent/tests/mcp_tools.rs` (фича): `initialize`, `tools/list` (пять инструментов со схемами),
  `tools/call` на фикстуре совпадает с `ToolRunner`, шаги, узлы и гипотезы собраны, неизвестный инструмент —
  ошибка JSON-RPC.
- [ ] `crates/app/tests/claude_code.rs` (фича): настоящая прослойка `domovoi --mcp-proxy` между stdin/stdout и
  сокетом теста; фейковый `claude` пишет аргументы и отвечает JSON — в аргументах все флаги Stack, с `session_id`
  — `--resume`, ответ и `session_id` разобраны, отмена завершает процесс.
- [ ] `bundle_profile.rs`: тест — фича есть в `dev` и `adhoc`, нет в `release`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-agent --features claude-code --test mcp_tools
```
- REAL-DEP: MCP отдаёт настоящие инструменты ToolRunner (exit 0).
```
cargo test -p domovoi-app --features claude-code --test claude_code
```
- Прослойка, флаги, `--resume`, отмена (exit 0).
```
cargo test -p xtask -- bundle_profile
```
- Claude только в сборке разработчика (exit 0).
```
cargo clippy -p domovoi-agent -p domovoi-app -p domovoi-eval -p xtask --all-targets --features domovoi-app/claude-code,domovoi-eval/claude-code -- -D warnings
```
- Typecheck-критерий фазы с фичей, включая провайдер eval (exit 0).

### Phase 5: claude-app — выбор Claude в приложении

<!-- deps: [claude-core] — потребление: AskJob запускает настоящий ClaudeCode. -->

- [ ] `AssistantSettings::provider`, поиск `claude`, поле модели экрана и переключатель «Claude через Claude Code»
  на «Модели», доступность «готов» без модели, `AskJob` через `ClaudeCode`, `session_id` в чате, «Стоп» по Stack.
- [ ] `crates/app/tests/claude_provider.rs` (фича): выбор провайдера сохраняется и читается; поиск находит
  `claude` в подставленных каталогах и молчит без него; при Claude доступность «готов» без модели; REAL-DEP — вопрос
  через `AskJob` с настоящим `ClaudeCode` и фейковым `claude`: ответ в чате, `session_id` сохранён, второй вопрос
  идёт с `--resume`, «Стоп» завершает процесс и пишет «Ответ остановлен.».

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --features claude-code --test claude_provider
```
- REAL-DEP: вопрос из приложения идёт через настоящий ClaudeCode (exit 0).
```
cargo test -p domovoi-app --test snapshot_ai
```
- Экраны ассистента и модели без фичи прежние (exit 0).
```
cargo test -p domovoi-app --test voice
```
- Новые строки проходят правила голоса (exit 0).
```
cargo clippy -p domovoi-app --all-targets --features domovoi-app/claude-code -- -D warnings
```
- Typecheck-критерий фазы с фичей (exit 0).

### Phase 6: wire — история на главном пути бандла

<!-- deps: [agent-context, agent-tools, eval-turns, claude-core, claude-app] — потребление ВСЕХ code-фаз: wire-терминал по WIRE-PHASE-RULE. -->

- [ ] `ScenarioStep::AskAssistant` ведёт один чат на сценарий; отчёт — `history_turns`.
- [ ] `wire_main_path.rs`: второй `ask_assistant` после первого — `history_turns == 1`, ответ получен.
- [ ] Прогрев гейта: `cargo xtask bundle --profile adhoc` (с фичей `claude-code`) выполняется задачей до гейта.
- [ ] `docs/SPEC-DOMOVOI.md`: [AI] — история, фокус и выбранные карточки в каждом вопросе; Claude через Claude
  Code только в сборке разработчика; `docs/ASSISTANT-OPTIONS.md` — статус.

**Done when (EXECUTABLE):**
```
cargo xtask bundle --profile adhoc
```
- Бандл для e2e собран (exit 0).
```
cargo test -p domovoi-app --test wire_main_path
```
- Второй вопрос в бандле идёт с историей первого (exit 0).
```
cargo xtask check-sources
```
- Согласованность документов (exit 0).
```
cargo clippy --workspace --all-targets -- -D warnings
```
- Весь workspace чист (exit 0).
