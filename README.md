# Phocida

Узкая IDE для Free Pascal и Lazarus. Это форк [Zed](https://github.com/zed-industries/zed),
из которого убрано лишнее и добавлено своё; кодовая база редактора, рендер и агент — апстримовые.

Цель другая, чем у Zed: не «редактор для всего», а инструмент, который открывает папку
Lazarus-проекта и даёт в ней работать — сборку, отладку, подсветку форм и контекст для агента —
без ручной настройки.

## Что уже работает

- **Pascal как встроенный язык**, не расширение: `tree-sitter-pascal` с патчами под FPC/LCL,
  подсветка, отступы, outline, текстовые объекты, скобки. Языковой сервер — `pasls`
  (вендорный), с путями к исходникам FPC и LCL, иначе переход к определению внутри LCL не работает.
- **`.lfm` как отдельный язык**: outline дерева компонентов с классами, разные роли у значения
  свойства (`Position = poDefault`) и обработчика (`OnClick = FormCloseQuery`), автоотступ,
  прыжки по `[ ] ( ) < > { }`.
- **`.lpi`/`.lpk` как отдельный язык**: structure проекта читается — подсветка тегов и
  атрибутов, отступы, outline по package/unit/целевым файлам; по открытому `.lpi` задачи
  `lazbuild` на каждый build mode. Редактора опций сборки нет — файл только читается.
- **Сборка**: задачи `fpc` по одному файлу и `lazbuild` по `.lpi` (`ctrl-shift-t`).
- **Отладка**: `F5` → gdb через DAP — останова, фреймы, локалы, `evaluate`, stdout процесса.
- **Контекст для агента**: `.zed/rules` — первый источник правил проекта, важнее корневых
  `AGENTS.md`/`.rules`. Шаблон правил под Lazarus-конвенции лежит в `../templates/lazarus-project/.zed/rules`.
- **Никаких предложений поставить расширение** при открытии `.pas`: авто-установка и подсказки
  выключены по умолчанию, конкурирующий языковой пакет из маркета не предлагается.
- **Узкий набор языков**: Pascal, `.lfm`, `.lpi`/`.lpk`, JSON/JSONC, Markdown, YAML. Встроенные
  адаптеры и грамматики Rust, Go, Python, TypeScript/JavaScript, C/C++, CSS, Bash и Tailwind
  вырезаны из бинаря; их LSP-обвязка и языковые задачи больше не собираются.
- **Собственное имя**: бинарник `phocida`, окно с классом `dev.phocida.Phocida`, настройки в
  `~/.config/Phocida`. Каталог апстримовых настроек не переносится.
- **Статистика никуда не уходит**: у телеметрии вырезан транспорт — события пишутся только в
  `~/.local/share/Phocida/logs/telemetry.log`; minidump'ы загружать нечем и некуда; запрос
  обновлений не несёт идентификаторов установки. Флаги `telemetry.*` остались в схеме настроек,
  но влияют только на локальный файл.

## Чего нет

- Дизайнера форм: `.lfm` редактируется только текстом.
- Своей сборки пакетов: macOS/flatpak/snap-обвязка апстрима не тронута, мы собираем из исходников.

## Сборка

Тулчейн жёстко запинен в `rust-toolchain.toml` (1.98.1, профиль minimal, таргеты `wasm32-wasip2`,
`wasm32-unknown-unknown`, `x86_64-unknown-linux-musl`). Зависимости на Arch-подобной системе:

```
pacman -S --needed gcc clang lld musl cmake alsa-lib fontconfig glib2 libva wayland \
  libgit2 libxcb libxkbcommon-x11 llvm openssl zstd pkgconf sqlite pipewire \
  xdg-desktop-portal jq git rustup
```

```
cargo build -j 4 --bin phocida      # debug, ~80 минут на четырёх ядрах с нуля
target/debug/phocida ~/мой-проект   # запускать от обычного пользователя, не от root
```

## Лицензия и происхождение

Форк ведётся от `zed-industries/zed`; база — `e21ff1b368`, ветка `phocida`. Код Zed распространяется
под Apache-2.0 или GPL-3 (`LICENSE-APACHE`, `LICENSE-GPL`). Вендорные компоненты остаются под своими
лицензиями: `vendor/pascal-language-server` (`COPYING`), `vendor/tree-sitter-pascal` (`LICENSE`);
`vendor/tree-sitter-lfm` и `vendor/tree-sitter-lpi` написаны в этом проекте.
