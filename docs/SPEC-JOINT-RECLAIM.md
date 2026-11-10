<!--
  SPEC-JOINT-RECLAIM.md — дельта к docs/SPEC-DOMOVOI.md: место, которое освобождают элементы, удаляемые вместе,
  считается одной суммой — для жёстких ссылок и для клонов. 4 фазы.

  Выбран вариант A из трёх (/mneme:plan от 09.11.2026): совместные удержания готовятся вместе с рекомендациями,
  лист «К удалению» и его замер считают по объединению элементов.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-RULES-FOLLOW-SETTINGS.md` (в рабочем дереве, ещё не в коммите).
- Prior spec-hash: sha256:<placeholder — confirm>
- Обоснование выбора — staged decision note (id после accept).
- Проблема. Размер считается по каждому элементу отдельно (`Index::reclaimable_estimate`,
  `crates/index/src/reclaimable.rs:163-180`): жёсткая ссылка, вторая запись которой лежит в другом элементе, уходит в
  `held_by_hard_links`, а клон — в `held_by_clones`, даже если все его партнёры удаляются вместе. Суммы отмеченного
  складывают числа элементов: сумма выбранного в «Рекомендациях» (`crates/app/src/card_model.rs:60-70`), «К удалению»
  (`crates/app/src/tray_model.rs:44-55`, `:146-148`) и точный замер листа (`crates/app/src/tray_measure_job.rs:25-40`).
  Красный тест `crates/app/tests/hard_linked_projects.rs`: два проекта с общим файлом-ссылкой, отмечены оба — 131072
  вместо 196608.
- Уже есть и не меняется: группы жёстких ссылок и потоков клонов индекса (`CopyGroups`, `crates/index/src/copy_groups.rs`)
  и их поддержка дельтой; точный размер только для выбранного (`TrayMeasureJob` в потоке); карточки — оценкой без
  системных вызовов; порядок и состав карточек.
- Не входит: «Дубликаты» — общие блоки копий, клонированных между собой, в «освободит» группы по-прежнему не
  засчитываются (`docs/DOMOVOI.md`, 4.5); снапшоты.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч (`crates/xtask/src/release.rs`,
  `crates/xtask/src/workspace_version.rs`).

# Stack

- `crates/index/src/reclaimable.rs`: счёт по набору узлов — `Index::reclaimable_estimate_of(&[NodeId])`,
  `Index::reclaimable_of(&[NodeId])` (точный, с `PRIVATESIZE`) и `measure_reclaimable_of(&[&Node])` (замер обхода);
  `reclaimable_estimate`, `reclaimable` и `measure_reclaimable` одного узла — это они же с одним узлом. Внутри —
  один `sum_over_files` по объединению поддеревьев (вложенные узлы набора не дублируются). Группа жёстких ссылок
  освобождается, когда в наборе все `link_count` её записей; поток клонов — когда в наборе все его ссылки, и тогда
  его общие блоки засчитываются один раз.
- `crates/index/src/held_group.rs` (новый): `HeldGroup` — ключ группы (ссылки или поток), байты, сколько её членов
  внутри элемента и сколько всего; `Index::held_groups(NodeId)` — группы, которые элемент держит не целиком.
- `crates/rules/src/card_item.rs`, `crates/rules/src/project_item.rs`: у элемента — его `held: Vec<HeldGroup>`.
- `crates/rules/src/joint_hold.rs` (новый): `JointHold { items: Vec<NodeId>, bytes }` — группа, все члены которой
  лежат внутри нескольких элементов карточек и проектов; `Recommendations.joint: Vec<JointHold>`.
- `crates/rules/src/claim_ledger.rs`: `settle` берёт у элемента оценку и `held_groups` одним проходом;
  `into_recommendations` сводит группы элементов по ключу в `joint`; дельта (`crates/rules/src/recommendations_delta.rs`)
  переносит `held` нетронутых элементов, пересчитывает задетые и сводит `joint` заново.
- `crates/app/src/recommendations_model.rs`, `crates/app/src/card_model.rs`, `crates/app/src/project_row.rs`:
  совместные удержания переводятся в единицы отметки (карточка без строк или строка проекта);
  `RecommendationsModel::selected_total()` прибавляет удержание, когда отмечены все его единицы; строка или
  карточка с 0 Б отмечается, если входит в совместное удержание.
- `crates/app/src/tray_model.rs`: итог и строки удержания листа — `index.reclaimable_estimate_of` по всем элементам
  списка, строки — по элементам; `crates/app/src/tray_measure_job.rs`: `TrayMeasured` несёт и итог
  `measure_reclaimable_of` по всем обойдённым корням.
- `docs/DOMOVOI.md`: абзац о `reclaimable` (раздел 4.4), правило 0 Б в «Проектах разработки», итог «К удалению».

# Conventions

- Одно правило групп для оценки, точного счёта и замера — `sum_over_files` и ключи `copy_groups`; одиночный элемент —
  частный случай набора, отдельного кода для него нет.
- Числа строк поэлементные: строка показывает, что освободит её удаление одной; совместное — только в суммах.
- Карточки и дельта — без системных вызовов; `PRIVATESIZE` — только в точном счёте выбранного.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p <крейты фазы> --all-targets -- -D warnings`; формат — `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test hard_linked_projects`:
    panicked at crates/app/tests/hard_linked_projects.rs:74:5:
    deleting both projects frees their own files and the file they share
    left: 131072
    right: 196608
- **Почему заранее.** Отметки переключаются без сессии (`ToggleCard` не ждёт её, `crates/app/tests/session_gate.rs`),
  поэтому сумма выбранного не может обойти индекс в момент клика — совместные удержания готовятся там, где элементы
  уже обходятся: `settle` в `crates/rules/src/claim_ledger.rs` и дельта.
- **pnpm и bun.** Файлы `node_modules` — ссылки или клоны из общего хранилища; пока хранилище не отмечено, его член
  лежит вне элементов, и группа в `joint` не попадает. `held` элементов при этом есть у каждого такого файла —
  память на элемент растёт с числом его файлов; время сборки держит бюджет `recommendations_scale`.
- **Поток клонов.** Оценка берёт число членов потока из индекса (`shares_with + 1`); замер обхода индекса не имеет и
  берёт число ссылок из `CLONE_REFCNT`. Что `CLONE_REFCNT` двух клонов равен 2 — не проверено, проверяется в фазе 1
  на настоящих клонах `cp -c` в `/private/tmp`. Поток, у членов которого разный `ALLOCSIZE` (клон дописан после
  копирования), в оценке засчитывается один раз по наибольшему `ALLOCSIZE`.
- **0 Б.** Правило `b29f150` («строка с 0 Б не отмечается») уточняется: строка, которая ничего не освобождает одна,
  но входит в совместное удержание, отмечается — иначе проект только из общего файла нельзя отметить вместе с
  партнёром; так и описано ожидание в исходной жалобе («при отметке обоих … удаление обоих освобождает место»).
- **Замер папки.** В `crates/index/tests/measured_reclaimable.rs:20-48` пары `twin` и `packed` лежат внутри `item` и
  теперь засчитываются один раз; `lonely` с партнёром снаружи держит `held_by_clones > 0` — проверка остаётся.
- **Прошлый опыт.** Заметки a848ca01 и 4707eeeb — одна группировка по иноду для точного счёта, оценки и замера, её
  и расширяем; 8419acb8 — неверную цифру чинить в модели, а не на экране; ed6767da — карточки на оценке без
  системных вызовов.
- **Граница wire.** Сценарий бандла проверяет, что сумма выбранного и итог «К удалению» на настоящих модулях
  совпадают со счётом по объединению; что видит пользователь на кнопке и в листе — приёмка руками на границе фазы
  wire. Бандл пересобирается перед тестом (`docs/SPEC-TRAY-MARKS.md:182-190` — тот же порядок).

# Gameplan

<!-- 4 фазы: счёт по набору в индексе → совместные удержания в rules → суммы в app → wire. -->

### Phase 1: set-reclaimable — счёт освобождения по набору узлов в индексе

- [ ] `reclaimable_estimate_of`, `reclaimable_of`, `measure_reclaimable_of` по набору; одиночные функции — через
  них; правило жёстких ссылок и потоков клонов по объединению.
- [ ] `HeldGroup` в своём файле и `Index::held_groups`.
- [ ] Тесты в `crates/index/tests/reclaimable.rs` на настоящих ссылках и клонах `cp -c`: пара в двух папках по
  отдельности и вместе, пара внутри одной папки, партнёр вне набора; `CLONE_REFCNT` пары; в
  `crates/index/tests/measured_reclaimable.rs` — замер набора равен точному счёту того же набора.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-index --test reclaimable
```
- Ссылки и клоны по набору: вместе освобождают, по отдельности держат (exit 0).
```
cargo test -p domovoi-index --test measured_reclaimable
```
- Замер обхода набора равен точному счёту индекса по тому же набору (exit 0).
```
cargo clippy -p domovoi-index --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).

### Phase 2: joint-holds — совместные удержания в рекомендациях (потребляет фазу 1: held_groups и счёт по набору)

- [ ] `held` у `CardItem` и `ProjectItem`; `JointHold` в своём файле и `Recommendations.joint`; сведение в
  `into_recommendations`; перенос и пересчёт в дельте.
- [ ] В `crates/rules/tests/recommendations_delta.rs` — дельта, задевшая один из проектов с общим файлом-ссылкой и
  с общим клоном, даёт те же `joint`, что полная сборка.
- [ ] В `crates/rules/tests/recommendations_scale.rs` — случай хранилища: файлы `node_modules` — ссылки на
  неотмеченное хранилище; `joint` пуст, сборка в прежнем бюджете.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test recommendations_delta
```
- Совместные удержания после дельты равны полной сборке на настоящем индексе (exit 0).
```
cargo test -p domovoi-rules --test recommendations_scale
```
- Сборка с удержаниями укладывается в бюджет, хранилище не даёт ложных удержаний (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-rules --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).

### Phase 3: joint-sums — суммы отмеченного считают вместе (потребляет фазы 1-2: счёт по набору и Recommendations.joint)

- [ ] Единицы удержаний в `RecommendationsModel`, `selected_total` с удержаниями, отметка строки и карточки с 0 Б в
  удержании; итог и удержание листа `TrayModel` по набору; итог набора в `TrayMeasured`.
- [ ] Абзацы в `docs/DOMOVOI.md`: `reclaimable` по набору (4.4), правило 0 Б в «Проектах разработки», итог
  «К удалению».
- [ ] В `crates/app/tests/hard_linked_projects.rs` — сумма выбранного, проект только из общего файла (0 Б, отмечается
  вместе с партнёром), итог `TrayModel::estimated` и итог `TrayMeasureJob` по двум проектам; в
  `crates/app/tests/smoke_tray_measure.rs` — итог набора равен сумме строк, когда общего нет.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test hard_linked_projects
```
- Отмеченные вместе проекты, лист и его замер засчитывают общий файл один раз (exit 0).
```
cargo test -p domovoi-app --test smoke_tray_measure
```
- Замер листа без общих файлов не изменился (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).

### Phase 4: wire — совместный счёт на главном пути бандла (потребляет фазы 1-3)

- [ ] Шаг сценария `--e2e` по рецепту `.claude/skills/wire-phase/SKILL.md`: два проекта с общим файлом-ссылкой на
  образе, отметка обоих, сумма выбранного и итог «К удалению» — через функции приложения; место и проверки в
  `wire_main_path`; абзац фазы в `docs/DOMOVOI.md` и образец книги дизайна, если меняется вид.

**Done when (EXECUTABLE):**
```timeout=900s
cargo xtask bundle --profile adhoc
```
- Бандл собирается (exit 0).
```timeout=900s
cargo test -p domovoi-app --test wire_main_path
```
- Бандл проходит главный путь, совместный счёт на настоящих модулях (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
