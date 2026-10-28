<!--
  SPEC-ANSWER-TARGETS.md — дельта к docs/SPEC-DOMOVOI.md: модель может назвать, что отметить к удалению, а кнопку под
  ответом строят правила. 2 фазы.

  ГРАНИЦА РЕШЕНИЯ (веер от 28.10.2026, выбран вариант A по рекомендации): необязательное поле targets финального
  ответа локальной модели и инструмент MCP mark_for_deletion у Claude; если цели названы, кнопки строятся только из
  них, иначе — прежним выводом из узлов инструментов. Каждый путь проходит ту же проверку: индекс, правило не
  «проверьте сами» и не справочное, denylist, без вложенных дублей. Отвергнуты: B — только цели модели (один источник,
  но compact-модели часто не заполняют поле, и кнопки пропадают там, где работают сейчас); C — цели уточняют догадку
  (цель заменяет выведенного предка; неочевидная семантика соседей и целей вне результатов инструментов).
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-CARD-QUESTION.md`
  (влита в `main`, `44d2d1b`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Кнопка «Отметить к удалению» под ответом строится только из узлов, которые вернули инструменты, а
  вложенные узлы отбрасываются, если в списке есть их предок (`agent_answer.rs::suggested_actions`). Разбор хранилища
  pnpm вызывает `inspect_node` на всё хранилище — кнопка выходит на 14,7 ГБ, даже когда модель советует удалить только
  `v3` (11,0 ГБ). Сказать «отметь только v3» модели нечем: текст ответа действий не создаёт.
- Уже есть и не меняется: `RuleSet::claim_of` (правило ищется и по предкам — `v3` наследует `cache.pnpm-store`),
  фильтры действия (уровень не «проверьте сами», правило не справочное, `Denylist::verdict` пуст), `home_path::resolved`,
  `ChatModel::record_answer` (действия → `AnswerItem` по правилу и `AnswerTarget` по пути), кнопка в чате и
  `add_answer_to_tray`, `note_hypothesis` в `McpSession`, гипотезы в финальном ответе.
- Версия: фазы повышения версии нет — `cargo xtask release` (`crates/xtask/src/release.rs`) сам инкрементирует патч;
  `.github/workflows/ci.yml` версию не меняет.
- Typecheck-критерий проекта: `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Обоснование выбора — staged decision note (id после accept).

# Stack

- `crates/grammar/src/response_schema.rs`: в `final_answer_schema` необязательное свойство `targets` — массив строк
  (`minLength` 1, `maxLength` 1024), `maxItems` `FinalAnswer::MOST_TARGETS = 5`; разбор кладёт его в
  `FinalAnswer.targets: Vec<String>` (пустой, если поля нет). `crates/grammar/tests/golden/response.gbnf` обновляется
  вручную под новую грамматику (`tool-calls.gbnf` не меняется: в ней нет финального ответа).
- `crates/agent/src/agent_answer.rs`: `AgentAnswer::assembled` строит действия из `answer.targets`, если они не пусты
  (`home_path::resolved` → `index.find`), иначе из узлов шагов, как сейчас; проверка узла — одна функция для обоих
  источников (правило через `claim_of`, уровень, справочность, denylist), вложенные дубли и сортировка по размеру —
  общие.
- `crates/agent/src/mcp_session.rs`: инструмент `mark_for_deletion` со схемой `{ path }`; путь вне индекса или без
  годного правила — `isError` с причиной, чтобы модель исправилась; больше `MOST_TARGETS` — `isError`; `answered`
  передаёт собранные цели в `FinalAnswer`.
- `crates/agent/src/instructions.rs`: строка «Если пользователь просит отметить к удалению конкретные папки, перечисли
  их полные пути в targets (у Claude — инструмент mark_for_deletion); кнопку покажет приложение, если правила это
  разрешают».
- `crates/agent/tests/answer_targets.rs` (новый), `crates/agent/tests/mcp_tools.rs` (фича `claude-code`),
  `crates/grammar/tests/response.rs`.
- `crates/app/tests/answer_targets.rs` (новый): настоящий `Agent::ask` на `FakeBackend` → `ChatModel::record_answer`.
- `docs/DOMOVOI.md` 4.6: правило старшинства «названное моделью важнее выведенного из инструментов».

# Conventions

- Модель выбирает узел, правила решают: уровень, справочность и denylist проверяются для названного пути так же, как
  для выведенного; модель не может поднять уровень или обойти denylist.
- Названные цели заменяют вывод полностью, а не дополняют его — иначе предок снова поглотит цель.
- Пути — данные: проверка через индекс, ничего не выполняется по тексту пути.
- Голос: строки инструкций и описание инструмента без восклицаний.

# Knowledge

- Поле `targets` необязательно, поэтому старые ответы без него остаются корректными: правка тестовых ответов не
  нужна, меняется только эталонная `response.gbnf` (необязательные свойства конвертер рендерит цепочкой
  `final-answer-tail-N`).
- Пример: цель `~/Library/pnpm/store/v3` — `claim_of` находит `cache.pnpm-store` на предке, уровень «восстановимо»,
  кнопка «Отметить к удалению · 11,0 ГБ» на одном `v3`. В фикстуре агента аналог — `engine/target/debug` внутри
  `engine/target` (`cargo.target`).
- Отброшенная цель молча не превращается в кнопку; у Claude инструмент возвращает причину сразу, у локальной модели
  поле финальное и исправить его некогда — это честное различие провайдеров.
- Сквозной тест на бандле этого не докажет: tiny-random заполняет поле случайно. Связь приложения доказывает тест на
  настоящем агенте с `FakeBackend`; живой разговор «отметь только v3» на настоящей модели и через Claude Code — приёмка
  человеком на границе фазы wire.

# Gameplan

### Phase 1: answer-targets — цели в ответе модели и кнопки из них

- [ ] Грамматика: `targets` в финальном ответе, `FinalAnswer.targets`, `MOST_TARGETS`, эталон `response.gbnf`, тесты
  разбора с полем и без.
- [ ] Агент: действия из целей при их наличии, общая проверка узла, строка инструкции.
- [ ] MCP: `mark_for_deletion` с проверкой и пределом, цели в `answered`.
- [ ] `agent/tests/answer_targets.rs`: цель `engine/target/debug` → одно действие на `debug`; цели вне индекса,
  «проверьте сами» и под denylist отброшены; без целей — прежние действия из шагов; цель и её предок — одно действие на
  предка.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-grammar --test converter
```
- Эталонные грамматики с полем targets (exit 0).
```
cargo test -p domovoi-grammar --test response
```
- Разбор финального ответа с targets и без (exit 0).
```
cargo test -p domovoi-agent --test answer_targets
```
- Кнопки из названных целей и прежний вывод без них (exit 0).
```
cargo test -p domovoi-agent --features claude-code --test mcp_tools
```
- Инструмент mark_for_deletion у Claude (exit 0).
```
cargo clippy -p domovoi-grammar -p domovoi-agent --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo clippy -p domovoi-agent --features claude-code --all-targets -- -D warnings
```
- Typecheck-критерий фазы с фичей Claude (exit 0).

### Phase 2: wire — цель модели до кнопки в чате

<!-- deps: [answer-targets] — потребление: FinalAnswer.targets и AgentAnswer.actions из целей. -->

- [ ] `app/tests/answer_targets.rs`: настоящий `Agent::ask` на `FakeBackend` с финальным ответом `targets` →
  `ChatModel::record_answer` → у записи ответа один `AnswerTarget` на названный путь и сумма `AnswerItem` равна его
  размеру; без targets — прежняя кнопка на выведенный узел.
- [ ] `docs/DOMOVOI.md` 4.6: правило старшинства и инструмент `mark_for_deletion`.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test answer_targets
```
- Названная цель доходит до кнопки в чате (exit 0).
```
cargo xtask check-comments
```
- Комментарии без ссылок на книгу, фазы и спеки (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
