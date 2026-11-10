# Веха 2 — «Лицо Lazarus»: тулбар с кнопками

Дата: 2026-11-10 · Ветка: `phocida`

## Зачем

Phocida выглядит и ощущается как Zed: тонкий титлбар, панели-доки, никаких кнопок. Целевой
пользователь — Pascal-разработчик, привыкший к Lazarus: меню сверху, под ним ряд мелких
иконок-кнопок (New/Open/Save/Save all/Build/Run), понятных с первого взгляда без чтения
документации.
После вехи: окно Phocida имеет классический тулбар под строкой меню, кнопка Build запускает
`lazbuild` по открытому `.lpi`, Run — собирает и запускает приложение.

Это первая часть большого направления «визуально как Lazarus» (тулбар → статус-бар →
раскладка панелей как в Lazarus → Object Inspector). Остальные части — отдельные вехи,
здесь их нет.

## Решение

### Подход (выбран A2 из двух)

**A1: вторая строка внутри существующего `TitleBar`** — отклонён: тулбар срастается с
титлбаром Zed-стиля (настройки титлбара скрывают и кнопки), крейт `title_bar` тащит лишний
вес (collab, update, onboarding), смешение ответственности.

**A2 (выбран): новый крейт `toolbar` + слот `toolbar_item` в workspace** — точная копия
паттерна `title_bar::init`/`set_titlebar_item`: независимая строка, свой focus-регион,
свои actions, свой крейт. Переживает настройки титлбара; в будущем — своё «показать/скрыть»
в настройках. Цена — ~40 строк plumbing'а в `workspace.rs` по образцу готового.

Факт из проверки билдера: в этом форке `title_bar::init` в приложении не вызывается
(остался только в `crates/zed/src/visual_test_runner.rs`), `titlebar_item` всегда `None` —
у окна нет титлбара и строки меню вообще. Тулбар становится самой верхней строкой окна —
это и есть целевой вид этой вехи. Меню-строка — отдельная будущая веха направления.

### Состав тулбара (v1)

Ряд мелких кнопок-иконок с тултипами, слева направо, группы разделены небольшим отступом:

| # | Кнопка | Иконка (icons-крейт) | Действие |
|---|--------|----------------------|----------|
| 1 | New file | `FilePlus`? — см. «что осталось билдеру» | `workspace::NewFile` |
| 2 | Open | `Folder` | `workspace::Open` |
| 3 | Save | `Save` | `workspace::Save` |
| 4 | Save all | `Files`? | `workspace::SaveAll` |
| 5 | Build project | `Hammer`? | новое action `toolbar::BuildProject` (ниже) |
| 6 | Run | `Play` | новое action `toolbar::RunProject` (ниже) |

Кнопки — `ui::IconButton` c тултипом; disabled-состояния: Save/Save all всегда активны
(как в Lazarus); Build/Run активны всегда — если `.lpi` задач нет, кнопка даёт
notification «открой .lpi проекта» (проще и честнее, чем следить за активным буфером).

### Build и Run

Тегирование: шаблоны lazbuild из `.lpi`-провайдера получают тег `"lazbuild"` в
`TaskTemplate.tags` (правка `crates/languages/src/lpi.rs`).

**Build** — action `toolbar::BuildProject`: через новую функцию
`tasks_ui::spawn_task_awaited` (расширение `spawn_tasks_filtered`: тот же сбор
task_contexts/inventory/resolve, но вместо «запланировать и забыть» —
`workspace.schedule_resolved_task_with_completion`, существующий механизм
`ScheduledTaskResult::{Success,Failure,SpawnFailed,Cancelled}`; возвращает результат).
Предикат: тег `lazbuild` + шаблон без `--build-mode` (дефолтный режим). Если ни один шаблон
не совпал — модалка выбора задач (существующее поведение `spawn_tasks_filtered`,
оставляем).

**Run** — action `toolbar::RunProject`: сначала тот же путь, что Build (сборка,
с ожиданием результата). При `Success` — запускает собранный exe как ещё одну задачу
через тот же `tasks_ui::spawn_task_awaited` с синтетическим шаблоном
`TaskTemplate { command: <exe>, cwd: директория .lpi }` — exe выполняется в панели
терминала так же, как сборка, с вкладкой и перезапуском. При `Failure`/`Cancelled` —
notification с ошибкой сборки, exe не запускается.

**Имя exe** — из открытого буфера `.lpi`: тулбар находит активный `.lpi`, парсит его
нашим `lazarus_project::parse`, берёт stem главного юнита (`project1.lpr` →
`project1.exe`). Нет главного юнита — fallback: stem самого `.lpi` + `.exe` (то же
приближение, что в `pascal_task_context`). Чистая функция `exe_name_from_lpi` живёт
в крейте `toolbar`, тест на `project1.lpi` → `project1.exe` там же. Для этого
`lazarus_project` добавляется в зависимости `toolbar` (крейт-лист, соответствующе
расширяется).

## Non-goals

- Pause/Stop/Step (отладка) — debug-адаптеры вырезаны; вернутся отдельной вехой про отладчик.
- Выбор build mode кнопкой (выпадающий список) — потом, вместе с редактором build modes.
- Статус-бар, Object Inspector, перестановка панелей — следующие вехи направления.
- Настройка состава тулбара пользователем (только показ/скрыть, и то потом).

## Constraints

- Паттерн в точности как `title_bar`: `init(cx)` + `observe_new` на `Workspace`,
  `workspace.set_toolbar_item(item.into(), window, cx)` — новый публичный слот рядом с
  `set_titlebar_item` (workspace.rs:3434), рендер в колонке окна сразу под титлбар-регионом
  (workspace.rs:9851 `when_some(self.titlebar_item...)`), свой focus_handle + region
  (`FocusablePart::toolbar`), ARIA-роль toolbar, tab_group, навигация стрелками — по
  образцу titlebar-региона.
- Кнопки только через существующие actions (`workspace::NewFile/Open/Save/SaveAll`) —
  тулбар не реализует логику сам, он только триггерит.
- Иконки — из существующего крейта `icons`; если точной иконки нет, ближайшая по смыслу
  из имеющихся (не тащить новые ассеты).
- Крейт `toolbar`: GPL-3.0-or-later, `edition.workspace`, lints workspace — по образцу
  `crates/lazarus_project/Cargo.toml`; в members — по алфавиту (`toolbar` после `title_bar`... фактически между `theme` и `ui` — на место по сортировке).
- Зависимости крейта: `gpui`, `ui`, `icons`, `workspace`, `tasks_ui`, `anyhow`, `task`,
  `lazarus_project` (для имени exe), `smol`/`futures` по необходимости — никаких
  collab/client/etc. `project` не добавляем: сбор задач остаётся внутри `tasks_ui`.
- `pascal_task_context` не трогаем; эвристика у `.lpr` остаётся как есть.
- Ошибки (нет main unit, упавшая сборка) — через `workspace.show_notification`/toast,
  не panic; «нет задач» — модалка выбора задач (существующее поведение Phocida).

## Проверяемость

- Тест на тег: шаблоны `lpi.rs` содержат `"lazbuild"` в tags (модульный тест в `lpi.rs`).
- Тест Build-предиката: предикат фильтрации выбирает ровно шаблон без `--build-mode`
  (модульный тест, чистая функция предиката в `toolbar`).
- Тест имени exe: `project1.lpi` (main unit `project1.lpr`) → `project1.exe`; без
  main unit → stem `.lpi` + `.exe` (модульный тест `exe_name_from_lpi` в `toolbar`).
- `cargo test -p toolbar -p languages` зелёный; `cargo build -p zed` не регрессирует
  и без новых warning'ов.
- Визуальная проверка руками: запустить, открыть `.lpi`, нажать Build — в панели задач
  идёт сборка; нажать Run — сборка, затем вкладка терминала с запущенным exe.

## Что осталось билдеру (мои решения — проверить)

- Точные иконки из `icons`: соответствие 6 кнопок. Проверка билдера: в `icons` нет
  save-подобной иконки вовсе; `FileGeneric`/`Folder`/`ToolHammer`/`PlayFilled` есть —
  брать ближайшие по смыслу из имеющихся, новых ассетов не тащить.
- Дефолт: тулбар включён всегда (настройки показа — потом).

## Решения по итогам проверки билдера (2026-11-10)

- Меню-строки в приложении нет (титлбар не инициализируется) — тулбар верхняя строка,
  эта веха ничего не добавляет про меню.
- «Нет задач» при Build → модалка выбора задач (существующее поведение), не notification.
- Run = вариант 1 «честный Run» через `tasks_ui::spawn_task_awaited` (новая функция,
  расширение `spawn_tasks_filtered` на завершение через
  `schedule_resolved_task_with_completion`); exe запускается той же функцией как
  синтетическая задача в панели терминала.
