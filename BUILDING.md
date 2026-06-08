# Сборка и запуск LCM

LCM целится в Linux (семейство Debian для MVP). Разрабатывать удобно в Docker —
заодно это реалистичная среда, где реально работают системный trust store и
`update-ca-certificates`. Хост может быть любым (macOS/Windows/Linux).

## Вариант A — в Docker (рекомендуется)

### 1. Собрать dev-образ
```sh
docker compose build
```
Образ на базе `rust:1-bookworm` с инструментами для сборки, тестов и
интеграции (openssl, p11-kit, libnss3-tools, polkit, GTK4-заголовки на будущее).

### 2. Поднять контейнер и зайти в него
```sh
docker compose up -d
docker compose exec dev bash
```
Дерево исходников примонтировано в `/workspace`. Cargo-registry и `target/`
лежат в именованных volume'ах — пересборки быстрые, артефакты хоста и контейнера
не конфликтуют.

### 3. Собрать и прогнать тесты (внутри контейнера)
```sh
cargo build
cargo test
```

### 4. Попробовать CLI (внутри контейнера)
```sh
# сгенерировать тестовый self-signed CA
./scripts/make-test-ca.sh

# посмотреть метаданные сертификата (привилегии не нужны)
cargo run -p lcm-cli -- info scratch/test-ca.crt

# предпросмотр плана установки, без изменений
cargo run -p lcm-cli -- ca install scratch/test-ca.crt --dry-run

# реально установить в системный trust store.
# Контейнер уже под root, поэтому polkit/pkexec не требуется — LCM выполнит
# привилегированные операции напрямую.
cargo run -p lcm-cli -- ca install scratch/test-ca.crt

# проверить, что появилось
cargo run -p lcm-cli -- ca list
ls -l /usr/local/share/ca-certificates/        # должен быть lcm-test-ca.crt
openssl verify -CApath /etc/ssl/certs scratch/test-ca.crt

# удалить
cargo run -p lcm-cli -- ca remove --name test-ca
```

> Для production-сборки одного бинаря: `cargo build --release` →
> `target/release/lcm`.

## Вариант B — нативно на Linux-хосте

Нужны: Rust 1.74+, `pkg-config`, `libssl-dev`, `ca-certificates`, а для записи
в системный trust store — `policykit-1` (даёт `pkexec`).

```sh
cargo build --release
./target/release/lcm info path/to/ca.crt
./target/release/lcm ca install path/to/ca.crt   # покажет диалог polkit
./target/release/lcm ca list
```
Когда `lcm` запущен не от root, привилегированные операции уходят в helper
(`lcm helper`) через `pkexec` — polkit покажет системный диалог авторизации.

## GUI (Tauri + React)

Десктопный GUI `lcm-gui` — это Tauri v2 (Rust-бэкенд поверх `lcm-core`) с
React/Vite фронтом в тёмной теме (дизайн-система из `kamienclave-control-plane`).

### Превью фронтенда в браузере (без Rust, на mock-данных)
Быстрее всего посмотреть UI — обычный браузер. Когда `__TAURI_INTERNALS__`
отсутствует, приложение отдаёт демо-данные.
```sh
cd gui
npm install
npm run dev          # http://localhost:5173
# или продакшн-проверка:
npm run build        # tsc --noEmit + vite build → gui/dist
```

### Полноценный запуск десктоп-приложения (Linux + webkit)
Нужны системные зависимости Tauri (в dev-образе уже стоят: `libwebkit2gtk-4.1`,
`libgtk-3`, `libsoup-3`, …) и `lcm` в `PATH` (как привилегированный helper).
```sh
cd gui
npm install
npm run tauri dev    # собирает src-tauri и открывает окно
# сборка бинаря/инсталляторов:
#   1) сгенерировать иконки:  npm run tauri icon src-tauri/icon.svg
#   2) включить bundle.active в src-tauri/tauri.conf.json
#   3) npm run tauri build
```
> В Docker окно не отрисуется (нет дисплея) — `tauri dev` запускают на
> Linux-десктопе. В контейнере полезно лишь скомпилировать бэкенд:
> `cd gui/src-tauri && cargo check`.

Привилегии в GUI устроены как в CLI: под root операции выполняются напрямую,
иначе GUI вызывает `pkexec lcm helper --json`, и polkit показывает диалог
авторизации. Путь к helper можно переопределить через `LCM_HELPER`.

### Сборка `.deb`

```sh
make deb          # .deb под арх хоста → ./dist/
```
Под капотом: `npm run tauri build` (в конфиге `bundle.targets=["deb"]`), артефакт
копируется в `./dist/` (например `LCM_0.1.0_arm64.deb`). Релизная сборка тяжёлая;
первый раз ~3 мин компиляции, дальше инкрементально (кэш в volume `gui-target`).

### Сборка под другую архитектуру (amd64 на Apple Silicon и наоборот)

Кросс-компиляция GUI на Tauri/WebKit нативно непрактична, поэтому собираем под
**эмуляцией** через отдельный Docker-сервис `dev-amd64` (`platform: linux/amd64`)
с собственными volume'ами (чтобы arm64- и amd64-артефакты не смешивались):

```sh
make image-amd64  # один раз — собрать amd64 dev-образ (эмулируется)
make deb-amd64    # → ./dist/LCM_0.1.0_amd64.deb
```
> ⚠️ Под эмуляцией медленно: на Apple Silicon холодный amd64-билд ~20–40+ мин.
> Сильно ускоряет Rosetta: Docker Desktop → Settings → General →
> «Use Rosetta for x86/amd64 emulation». Имена `.deb` различаются по арх
> (`_arm64` / `_amd64`), так что обе сборки спокойно лежат в `./dist/` рядом.

## Версия

Единственный источник правды — файл **`VERSION`** в корне. Чтобы сменить версию:

```sh
make set-version VERSION=0.2.0     # или ./scripts/set-version.sh 0.2.0
```
Скрипт пишет версию в `VERSION` и синхронизирует её в `Cargo.toml`
(CLI/core), `gui/src-tauri/Cargo.toml` (крейт GUI) и
`gui/src-tauri/tauri.conf.json` (имя `.deb`/`.rpm` и версия приложения).
GUI читает `VERSION` напрямую через Vite при сборке — править строку в UI не
нужно. После `set-version` собирай пакет как обычно (`make deb` / `make packages`).

> Почему не «чистая ссылка» для Rust: Cargo требует, чтобы `[package] version`
> был литералом в `Cargo.toml`, поэтому версия туда **синхронизируется** из
> `VERSION`, а не читается на лету.

## Команды CLI (v1)

| Команда | Что делает | Нужен root |
|---|---|---|
| `lcm info <file>` | показать subject/issuer/срок/отпечаток (PEM или DER) | нет |
| `lcm ca install <file> [--name N] [--force] [--dry-run]` | установить CA в системный trust store | да |
| `lcm ca list [--json]` | перечислить установленные LCM-якоря | нет |
| `lcm ca remove --name N [--dry-run]` | удалить ранее установленный якорь | да |

Флаг `--dry-run` печатает план (что и куда будет записано, какая команда
применится) и ничего не меняет. `lcm helper` — внутренний привилегированный
режим, его не вызывают руками.

## Структура репозитория

```
linux-ca-manager/
├── Cargo.toml                  # Cargo workspace
├── crates/
│   ├── lcm-core/               # ядро: detection, backends, cert, plan, exec
│   │   └── src/
│   │       ├── osrelease.rs    # парсер /etc/os-release
│   │       ├── distro.rs       # определение семейства дистрибутива
│   │       ├── backend/        # trust-store backends (debian)
│   │       ├── cert.rs         # парсинг X.509 (PEM/DER)
│   │       ├── plan.rs         # модель привилегированных операций
│   │       └── exec.rs         # выполнение плана + аудит
│   └── lcm-cli/                # бинарь `lcm` (CLI + helper-режим)
├── gui/                        # десктоп GUI
│   ├── src/                    # React + Vite фронт (Tokyo Night)
│   │   ├── components/         # Layout, ui-кит, иконки
│   │   ├── pages/              # Overview, Trust
│   │   └── api/                # client (tauri | mock), типы
│   └── src-tauri/              # Tauri-бэкенд: команды поверх lcm-core
├── scripts/make-test-ca.sh     # тестовый CA для разработки
├── Dockerfile / docker-compose.yml
├── DESIGN.md / README.md / BUILDING.md
```

## Качество кода
```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```
