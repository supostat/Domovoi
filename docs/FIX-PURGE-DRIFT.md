<!--
  FIX-PURGE-DRIFT.md — дельта к docs/SPEC-DOMOVOI.md: проверка purge_space в wire_main_path иногда падает на
  freed > 0 на образе диска, где освобождать нечего. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 07.11.2026, выбрана гипотеза 1 по рекомендации): свободное место живого тома APFS меняется
  само, а freed по контракту — рост доступного места между двумя замерами задачи; проверка freed == 0 требует
  неподвижного тома. Не опровергнута контрольным прогоном гипотеза 2 — сам запрос CacheDelete на 0 байт освобождает
  несколько блоков; правка проверки одна для обеих, контрольный прогон фазы называет причину. На ревью: тест диагноза
  purge_on_image.rs удаляется после контрольного прогона — контракт уже доказывает purge_job.rs.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя — `docs/SPEC-WIRE-SKILL.md`
  (`a140d27` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. `assert_space_release` (`crates/app/tests/wire_main_path.rs`) требует `freed == 0` и
  `available_after >= available_before` на образе с оценкой 0. Без изменений кода проверка иногда красная:
  `freed 32768` в фазе wire spec-tray-marks, следующие четыре повтора на том же бандле зелёные.
- Уже есть и не меняется: `PurgeJob` (`crates/app/src/purge_job.rs`) — `freed = available_after.saturating_sub(
  available_before)`; продуктовые тесты `crates/app/tests/purge_job.rs`; шаг `purge_space` в
  `crates/app/src/end_to_end.rs`.
- Вне этой спеки: изменение продукта (не звать механизм при оценке 0, вычитать фоновый рост) — пользователь не
  видит пункта «Освободить…» при нулевом purgeable.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/app/tests/wire_main_path.rs`, `assert_space_release`: проверки `freed == 0` и
  `available_after >= available_before` заменяются одной `freed == available_after.saturating_sub(available_before)`;
  `error == null`, `estimate == 0`, `finish == completed` и проверка второго `hidden_space` остаются.
- `crates/app/tests/purge_on_image.rs` (написан диагнозом, не в git): служит контрольному прогону фазы и удаляется.

# Conventions

- Один тип — один файл; имена без сокращений; комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Никакого допуска в байтах: величина фонового роста тома не ограничена, любое число было бы угадыванием.
- Typecheck-критерий — `cargo clippy -p domovoi-app --all-targets -- -D warnings`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-app --test purge_on_image`, 2 красных из 9:
    panicked at crates/app/tests/purge_on_image.rs:36:5:
    nothing is freed where nothing is held: …
    release 7 estimate: 0, freed: 8192, available_before: 32825344, available_after: 32833536, finish: Completed
- **Исходный сбой.** Гейт фазы wire spec-tray-marks, попытка 1:
    panicked at crates/app/tests/wire_main_path.rs:746:5: nothing is freed where nothing is held:
    "estimate": 0, "freed": 32768, "available_before": 17276928, "available_after": 17309696, "finish": "completed"
- **Опрос без механизма.** Образ APFS 32m, `df -k` каждые 50 мс, механизм не вызывался:
    в покое 15 с: t=6500ms 32072 -> 32076 KB
    после удаления 40 файлов по 64 КБ и 4 МБ: t=100ms 32068 -> 32076 KB
- **Контракт.** `freed` — рост доступного места между замерами задачи (`purge_job.rs:109-112`); тесты
  `the_completion_callback_finishes_the_release_and_the_growth_is_what_was_freed` и
  `a_drop_in_the_available_space_counts_as_nothing_freed` (`purge_job.rs`). Окно между замерами длится до ответа
  механизма; пока ответа нет, цикл спит `AVAILABLE_POLL` 500 мс (`purge_job.rs:151`) — отложенное освобождение
  после удаления (100 мс) в него попадает.
- **Гипотеза 1 (выбрана).** Свободное место тома растёт и падает само (отложенное освобождение после удалений,
  служебные записи APFS); `freed == 0` и `available_after >= available_before` требуют неподвижного тома.
- **Гипотеза 2 (не опровергнута).** Запрос CacheDelete на 0 байт сам освобождает блоки — в опросе шаги были 4–8 КБ,
  в сбое за одно окно 32 КБ. Контрольный прогон фазы различает их; правка проверки та же.
- **Тест диагноза.** `purge_on_image.rs` не остаётся: новую проверку детерминированно доказывают тесты `purge_job.rs`
  на поддельном механизме, настоящий механизм на образе — `assert_space_release`; сам он ловит старую ошибку лишь
  вероятностно (2 из 9) и добавляет ещё один параллельный тест с образом — источник сбоя `ImageVolume::create`
  «the image attaches» (заметка 46dd48ef).
- **Предел проверки.** После правки главный путь доказывает цепочку dlsym → оценка → запуск → обратный вызов →
  итог по росту SPACEAVAIL и согласованность отчёта, но не то, что механизм на образе ничего не освободил: это
  неотличимо от фонового роста тома.

# Gameplan

<!-- 1 фаза; гейт — контракт задачи, главный путь бандла с новой проверкой. -->

### Phase 1: purge-drift — проверка освобождения не требует неподвижного тома

- [ ] Контрольный прогон: в `purge_on_image.rs` тот же цикл с поддельным механизмом (оценка 0, ответ сразу) и
  настоящим `container_capacity`, 3 прогона; итог — бывает ли `freed > 0` без механизма — в заметку фазы с цитатой
  вывода.
- [ ] Удалить `crates/app/tests/purge_on_image.rs`.
- [ ] `wire_main_path.rs`, `assert_space_release`: замена проверок по Stack.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-app --test purge_job
```
- Контракт задачи: freed — рост доступного места, падение — ноль (exit 0).
```timeout=900s
cargo xtask bundle --profile adhoc
```
- Бандл собирается (exit 0).
```timeout=900s
cargo test -p domovoi-app --test wire_main_path
```
- Главный путь бандла с новой проверкой освобождения (exit 0).
```
cargo clippy -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
