<!--
  FIX-REINSTALLED-OWNER.md — дельта к docs/SPEC-DOMOVOI.md: после установки программы при открытом Домовом её данные
  остаются в «Данных удалённых программ», а после изменения её кэша он учитывается в двух карточках и двух итогах;
  удаление программы так же не переносит её данные в остатки до перезапуска. 1 фаза.

  ДИАГНОЗ (/mneme:fix от 09.11.2026, гипотеза 1, правка A из двух): индекс при смене набора программ пересчитывает
  владельцев всего дерева, но DeltaApplied об этом молчит, и Recommendations::apply_delta переносит старые заявки
  остатков; правка — флаг в DeltaApplied::Spliced и полная сборка рекомендаций по нему.
-->

# Baseline

- Предыдущая спека: `docs/SPEC-DOMOVOI.md`; все дельты из `docs/` применены, последняя —
  `docs/FIX-EMPTY-PROJECT-ROW.md` (`b29f150` в `main`).
- Prior spec-hash: sha256:<placeholder — confirm>
- Проблема. Программу удалили, её данные попали в «Данные удалённых программ». Программу поставили снова, не
  перезапуская Домового, — карточка осталась и предлагает удалить данные установленной программы; после изменения
  файла в её кэше кэш виден в двух карточках и в двух итогах уровней. Обратное направление сломано так же: удалённая
  при открытом Домовом программа не даёт остатков до полной пересборки.
- Уже есть и не меняется: правила остатков (`apps.leftover-caches`, `apps.leftover-data`, `Condition::OwnerMissing`);
  пересчёт владельцев и установки в индексе при смене набора бандлов; обычная дельта рекомендаций, когда набор
  программ прежний; ветка `Compacted`.
- Версия: фазы повышения версии нет — `cargo xtask release` сам инкрементирует патч.

# Stack

- `crates/index/src/delta_applied.rs`: `DeltaApplied::Spliced` получает поле `owners_reassessed: bool` — дельта
  изменила набор бандлов, и индекс заново назначил владельцев и установку всему дереву.
- `crates/index/src/apply_delta.rs`: `reassess_bundles` сообщает, изменился ли набор (сейчас при прежнем наборе он
  выходит сразу), `apply_delta` кладёт это в `Spliced`.
- `crates/rules/src/recommendations_delta.rs`, `Recommendations::apply_delta`: на `Spliced` с `owners_reassessed`
  рекомендации собираются целиком с удержанными гипотезами — тем же путём, что на `Compacted`.
- `crates/app/src/session.rs`: сопоставление `DeltaApplied` не меняется (`Spliced { .. }`).
- `crates/index/tests/delta_equivalence.rs`: флаг поднят, когда бандл появляется и исчезает, и опущен при обычном
  изменении файлов.
- `crates/rules/tests/reinstalled_owner.rs` (написан диагнозом): регрессия на дельте индекса и рекомендаций.
- `docs/DOMOVOI.md`, абзац «Остаток — папка…»: установка или удаление программы при открытом Домовом сразу
  пересчитывает остатки.

# Conventions

- Один путь: о смене владельцев правила узнают только из `DeltaApplied`; рекомендации не сравнивают состояние
  индекса сами.
- Комментарии не ссылаются на спеку и фазы (`cargo xtask check-comments`).
- Typecheck-критерий — `cargo clippy -p domovoi-index -p domovoi-rules -p domovoi-app --all-targets -- -D warnings`;
  формат — `cargo fmt --all -- --check`.

# Knowledge

- **Красный прогон.** `cargo test -p domovoi-rules --test reinstalled_owner` — 0 passed, 3 failed (left — дельта,
  right — полная сборка):
    removing_the_owner_puts_its_data_into_the_leftovers:
      left: {"cache.app.generic": {"Users/tester/Library/Containers/com.domovoi.tiny/Data/Library/Caches"}}
     right: {"apps.leftover-data": {"Users/tester/Library/Containers/com.domovoi.tiny"}}
    installing_the_owner_takes_its_data_out_of_the_leftovers:
      left: {"apps.leftover-data": {"Users/tester/Library/Containers/com.domovoi.tiny"}}
     right: {"cache.app.generic": {"Users/tester/Library/Containers/com.domovoi.tiny/Data/Library/Caches"}}
    a_cache_change_after_the_install_counts_the_cache_once:
      left: {"apps.leftover-data": {"…/Containers/com.domovoi.tiny"}, "cache.app.generic": {"…/Data/Library/Caches"}}
     right: {"cache.app.generic": {"Users/tester/Library/Containers/com.domovoi.tiny/Data/Library/Caches"}}
- **Причина.** `apply_delta.rs:100-102` → `reassess_bundles` → `assign_categories_and_owners` над всем деревом
  (`apply_delta.rs:236-245`), но `DeltaApplied::Spliced` несёт только `directories` и `regrouped`
  (`delta_applied.rs:5`). `Recommendations::apply_delta` переносит заявки из прежних карточек и проходит только
  каталоги пачки (`recommendations_delta.rs:23-41`): контейнер в пачку не попал, условие `OwnerMissing`
  (`rule_set.rs:271-274`) для него не перепроверяется.
- **Двойной учёт.** При изменении кэша `scope_of` (`rule_set.rs:134-152`) идёт от корня: контейнер теперь открыт,
  `Caches` получает `cache.app.generic`, `forget_within` чистит только `Caches`, а заявка `apps.leftover-data` на
  контейнер выше остаётся.
- **Индекс прав.** `delta_equivalence::a_bundle_that_appears_and_vanishes_moves_its_files_between_installed_and_leftover`
  зелёный; полная сборка на том же индексе даёт правую сторону.
- **Цена.** `reassess_bundles` выходит сразу, если набор бандлов прежний (`apply_delta.rs:238`); набор сравнивается
  с содержимым (`bundle_discovery.rs:11-33`), поэтому полная сборка бывает при установке, удалении, переименовании
  и обновлении программы.
- **Отвергнуто.** Правка B — индекс называет программы со сменившейся установкой, журнал заявок забывает и обходит
  их корни: больше кода и мест для ошибки, а `owned_roots` (`ownership.rs:49-60`) всё равно обходит всё дерево.
  Гипотеза «индекс не переводит программу в установленные» опровергнута тестом выше.
- **Проверка мутацией.** Одна мутация на прогон: убрать полную сборку по `owners_reassessed` в
  `Recommendations::apply_delta` — `reinstalled_owner` краснеет.

# Gameplan

<!-- 1 фаза: смена набора программ пересобирает рекомендации. -->

### Phase 1: reinstalled-owner — установка и удаление программы сразу пересчитывают остатки

- [ ] `DeltaApplied::Spliced::owners_reassessed` и его заполнение в `Index::apply_delta` по Stack; полная сборка в
  `Recommendations::apply_delta` по этому полю.
- [ ] `crates/index/tests/delta_equivalence.rs`: флаг поднят при появлении и исчезновении бандла и опущен при
  обычном изменении файлов.
- [ ] Абзац об остатке в `docs/DOMOVOI.md`: установка или удаление программы при открытом Домовом сразу
  пересчитывает остатки.
- [ ] Оставить `crates/rules/tests/reinstalled_owner.rs` регрессионным тестом; одна мутация по Knowledge.

**Done when (EXECUTABLE):**
```
cargo test -p domovoi-rules --test reinstalled_owner
```
- После установки и удаления программы дельта совпадает с полной сборкой, кэш учитывается один раз (exit 0).
```
cargo test -p domovoi-index --test delta_equivalence
```
- Дельта индекса совпадает с полной сборкой и сообщает о пересчёте владельцев только при смене набора бандлов (exit 0).
```
cargo test -p domovoi-rules --test recommendations_delta
```
- Обычная дельта рекомендаций не изменилась (exit 0).
```
cargo test -p domovoi-rules --test app_leftovers
```
- Остатки удалённых программ находятся по-прежнему (exit 0).
```
cargo clippy -p domovoi-index -p domovoi-rules -p domovoi-app --all-targets -- -D warnings
```
- Typecheck-критерий фазы (exit 0).
```
cargo fmt --all -- --check
```
- Формат совпадает с rustfmt (exit 0).
