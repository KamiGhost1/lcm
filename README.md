# LCM — Linux Cert Manager

**Берёт готовые серверные и клиентские сертификаты и встраивает их в нужные
места Linux-системы** — системный trust store, базы браузеров (NSS) и
расположения сервисов — одним действием, из GUI или CLI. Без ручного
копирования файлов и гугления команд под свой дистрибутив.

LCM — это «последняя миля» для сертификатов. Он **не выпускает** их сам: выпуск,
PKI, CRL и подпись — это [Secutor](https://github.com/KamiGhost1/secutor). LCM
берёт готовый материал (от Secutor, корпоративного CA, Let's Encrypt или `.p12`
от админа) и **разворачивает его в систему**.

```
  Secutor (выпуск/PKI)  ──файлы──▶  LCM (интеграция в Linux)
```

## Что умеет (v1)

- 🏛 **CA / доверие** — установка/удаление CA в системный trust store с
  автоопределением дистрибутива; опционально сразу в браузеры (NSS).
- 🪪 **Клиентские идентичности** — импорт `cert + key` (PEM/PKCS#12) в браузеры
  для mTLS.
- 🌐 **Серверные сертификаты** — разворачивание `cert + key + chain` в
  расположение сервиса (nginx / Apache / Traefik / haproxy), права, reload.
- ⏰ **Аудит** — что установлено, отпечатки, сроки и предупреждения об
  истечении; откат/удаление.

## Принципы

- **CLI и GUI равноправны.** Одно ядро `lcm-core`, тонкие фронты: `lcm` для
  серверов/CI и GTK4-приложение для десктопа.
- **Привилегии по правилам.** Запись в системные хранилища — через отдельный
  helper и **polkit**. Приложение целиком под root не запускается.
- **Никаких тихих изменений.** Перед применением показывается точный план:
  какие файлы и куда, какая команда. `--dry-run` в CLI.
- **Безопасность по умолчанию.** Приватные ключи — `0600` и отдельное
  хранилище, пароли — в системном keyring, секреты в памяти затираются.

## Пример (CLI)

```sh
lcm ca install corp-root.crt --system --nss          # CA в систему и браузеры
lcm client import alice.p12 --nss                     # клиентская идентичность для mTLS
lcm server deploy site.p12 --service nginx --reload   # серверный серт под nginx
lcm list --installed --expiring 30d                   # аудит и сроки
```

## Поддержка (v1)

- **Дистрибутивы:** Debian · Ubuntu · Mint · Pop!_OS · Fedora · RHEL · CentOS
  Stream · Rocky · Alma · Arch · Manjaro · openSUSE · Alpine
  (определение по `/etc/os-release`).
- **Браузеры:** общая `~/.pki/nssdb`, профили Firefox.
- **Сервисы:** nginx, Apache, Traefik, haproxy.

## Стек

- **Ядро `lcm-core` + CLI `lcm`** — Rust. Привилегированный helper (`lcm helper`)
  запускается транзитно через pkexec.
- **GUI `lcm-gui`** — Tauri v2 (Rust поверх `lcm-core`) + React/Vite, тёмная тема.
  React-фронт превьюится и в обычном браузере на mock-данных.

## Связь с Secutor (план)

Чтение `.skb`-бандлов и контекстов Secutor напрямую, чтобы путь «из Secutor в
систему» стал одним действием.

## Статус

🚧 Ранняя разработка (веха M1: ядро `lcm-core` + CLI `lcm` с установкой CA для
семейства Debian). Архитектура и детали — в [DESIGN.md](DESIGN.md).

## Сборка и запуск

Удобнее всего в Docker (заодно реалистичная Linux-среда для trust store):

```sh
docker compose build
docker compose up -d
docker compose exec dev bash
# внутри контейнера:
cargo test
./scripts/make-test-ca.sh
cargo run -p lcm-cli -- ca install scratch/test-ca.crt
```

GUI-превью в браузере (на демо-данных, без Rust):

```sh
cd gui && npm install && npm run dev   # http://localhost:5173
```

Полная инструкция (Docker, нативная сборка, GUI на Tauri, все команды CLI) — в
[BUILDING.md](BUILDING.md). Упаковка (AppImage/Flatpak/deb/rpm) — отдельная веха
позже.

## Лицензия

[Apache-2.0](LICENSE).
