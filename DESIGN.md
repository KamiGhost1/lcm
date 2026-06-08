# LCM — Linux Cert Manager · Дизайн-документ

> Берёт готовые серверные и клиентские сертификаты и **встраивает их в нужные
> места Linux-системы** — системный trust store, базы браузеров (NSS) и
> расположения сервисов — одним действием, из GUI или CLI, без ручного
> копирования файлов и гугления команд под свой дистрибутив.

Статус: **черновик v0.2** · Дата: 2026-06-08 · Лицензия: Apache-2.0

---

## 1. Позиционирование

LCM — это «последняя миля» для сертификатов на Linux. Он **не выпускает**
сертификаты сам: выпуск, иерархия CA, CRL, подпись — это работа
[Secutor](https://github.com/KamiGhost1/secutor). LCM берёт уже готовый
материал (откуда угодно: Secutor, корпоративный CA, Let's Encrypt, `.p12` от
админа) и **разворачивает его в систему**.

```
   Secutor (выпуск/PKI)            LCM (интеграция/разворачивание)
   ────────────────────  ──файлы──▶  ───────────────────────────────
   root/intermediate CA   PEM/p12     • системный trust store (CA)
   server / client leaves  bundle     • NSS браузеров (CA + client id)
   nginx/Traefik bundle               • расположения сервисов (server cert+key)
                                       • перезагрузка сервиса
```

Аналогия: если Secutor — это «выпускающий центр», то LCM — «Связка ключей»
Linux, которая раскладывает готовое по правильным полкам конкретного
дистрибутива.

## 2. Проблема

Положить готовый сертификат «куда надо» на Linux — это до сих пор консоль и
знание частностей:

- **Trust store фрагментирован.** Чтобы система доверяла твоему CA:
  | Семейство | Куда класть CA | Чем применять |
  |---|---|---|
  | Debian / Ubuntu / Mint | `/usr/local/share/ca-certificates/*.crt` | `update-ca-certificates` |
  | Fedora / RHEL / Rocky / Alma | `/etc/pki/ca-trust/source/anchors/` | `update-ca-trust` |
  | Arch / Manjaro | `/etc/ca-certificates/trust-source/anchors/` | `trust anchor` |
  | openSUSE | `/etc/pki/trust/anchors/` | `update-ca-certificates` |
  | Alpine | `/usr/local/share/ca-certificates/*.crt` | `update-ca-certificates` |
- **Браузеры игнорируют систему.** Firefox/Chrome/Electron смотрят в свою
  NSS-базу `~/.pki/nssdb` — туда нужен отдельный импорт через `certutil`/`pk12util`.
- **Клиентские идентичности (mTLS)** надо вручную скармливать каждому
  приложению/браузеру.
- **Серверные сертификаты** надо разложить cert+key в место, которое ждёт
  сервис, выставить владельца/права `0600`, поправить конфиг и перезапустить
  демон — каждый раз заново.

LCM прячет всё это за одним действием с автоопределением дистрибутива и
понятным предпросмотром того, что именно будет сделано.

## 3. Цели и не-цели

### Цели (v1)
- **Импорт** готового материала: PEM (`.crt/.pem/.cer`, комбинированный),
  PKCS#12 (`.p12/.pfx`), отдельные cert+key.
- **CA / trust:** установить/удалить CA в системный trust store с
  автоопределением дистрибутива; опционально — параллельно в NSS браузеров.
- **Клиентские идентичности:** установить пару cert+key в NSS (Chrome/Firefox)
  и/или в управляемое пользовательское хранилище для mTLS.
- **Серверные сертификаты:** развернуть cert+key+chain в расположение сервиса
  (nginx/Apache/Traefik/haproxy), выставить владельца и права, опционально
  перезагрузить сервис.
- **Два равноправных интерфейса:** GUI (GTK4) и CLI (`lcm ...`) — одно ядро.
- **Аудит:** показать, что уже установлено (системно и в NSS), отпечатки,
  сроки, предупреждать об истечении; уметь откатить/удалить установленное.
- **Привилегии через polkit** — никакого «всё под root».

### Не-цели (v1)
- Не выпускаем сертификаты, не ведём PKI, не делаем CRL/подпись — это Secutor.
- Не управляем SSH-ключами (отложено).
- Не делаем ACME-клиента/обновления Let's Encrypt.

### На потом (явные кандидаты)
- **Windows** — см. §3.1 (план), не входит в v1.
- **Нативная поддержка форматов Secutor:** читать `.skb`-бандлы и
  контексты-SQLite напрямую, чтобы «из Secutor в систему» было в один клик.
- Коннекторы VPN (NetworkManager, OpenVPN), SSH, авто-renew-хуки.

## 3.1 Windows (план, не v1)

Кодовая база уже подготовлена к кросс-платформенности: unix-специфичные куски
(права `0600`/`0644`, проверка прав через `geteuid`) вынесены за `#[cfg(unix)]`
в `lcm-core::util` (`set_mode`, `is_elevated`), GUI на Tauri собирается под
Windows из коробки (`.msi`/NSIS `.exe`). Что нужно доделать для боевого Windows:

| Аспект | Linux (есть) | Windows (план) |
|---|---|---|
| Trust store | `update-ca-certificates` и др. | системное хранилище **ROOT** через `CertAddEncodedCertificateToStore` (CryptoAPI) или `certutil -addstore Root` |
| Бэкенд-детект | `/etc/os-release` → `DistroFamily` | отдельный `WindowsBackend` (детект по `cfg(windows)`) |
| Привилегии | polkit / `pkexec lcm helper` | **UAC**: запуск helper'а через `runas`/`ShellExecuteEx` с `runas` (elevation prompt) вместо polkit |
| Клиентские идентичности | NSS / user-store | Windows Certificate Store (Personal) + опц. NSS для портативного Firefox |
| Серверные серты | nginx/Apache/HAProxy + reload | IIS (центральное хранилище сертов / `netsh http`), reload через `appcmd` |
| Права на ключ | `chmod 0600` | ACL (DPAPI / `icacls`) — `util::set_mode` уже no-op на не-unix, заменить на ACL |
| Поставка | deb/rpm/AppImage | `.msi` + NSIS `.exe` (Tauri умеет) |

Порядок: ① `WindowsBackend` для системного ROOT-store; ② elevation через UAC;
③ клиентские идентичности в Personal store; ④ (опц.) IIS-деплой.

## 4. Скоуп: объекты и что значит «интеграция»

Три типа объектов и их «места назначения»:

| Объект | Что это | Куда LCM его ставит (integration targets) | Нужен root? |
|---|---|---|---|
| **CA / trust anchor** | корневой/промежуточный сертификат | системный trust store; NSS браузеров | да (система) / нет (NSS) |
| **Client identity** | client cert + приватный ключ | NSS браузеров; управляемое user-хранилище | нет |
| **Server certificate** | server cert + key + chain | каталог сервиса (nginx/…); + reload | да |

«Интеграция» = положить объект туда, где его реально подхватит ОС/браузер/
сервис, с корректными правами и применением (`update-*` / `certutil` / reload),
а не просто сохранить файл.

## 5. Стек, интерфейсы, поставка

**Ядро: Rust.** Один статически слинкованный бинарь `lcm`, без рантайма;
сильная крипто-экосистема (`x509-parser`, `openssl`/`rustls`, FFI к `p11-kit`),
memory-safety для кода, который трогает приватные ключи и работает с root.

**Два интерфейса поверх одного ядра-библиотеки `lcm-core`:**
- **CLI** (`lcm`) — первоклассный, неинтерактивный, для серверов/скриптов/CI.
- **GUI** (`lcm-gui`) — **Tauri v2**: Rust-бэкенд вызывает `lcm-core` через
  `#[tauri::command]`, фронт — React + Vite + TypeScript. Дизайн-система (тёмная
  Tokyo Night) перенесена из `kamienclave-control-plane`. Один бинарь со всем
  внутри (webview системный), drag-and-drop сертификатов, предпросмотр плана.

Почему Tauri, а не GTK4: нужен конкретный визуальный язык control-plane (карточки,
бейджи сроков, модалки), который естественно делается на web-стеке; при этом
Rust-ядро переиспользуется напрямую, а итог — компактный самодостаточный бинарь
(возвращает идею «один файл»). React-фронт ещё и превьюится в обычном браузере на
mock-данных (выбор реализации — по наличию `__TAURI_INTERNALS__`).

Логика интеграции, backends, валидация и формирование «плана» живут в
`lcm-core`; CLI и GUI — тонкие фронты. Привилегированные операции оба гонят
через один и тот же helper (`lcm helper [--json]`).

**Поставка: пока из исходников** (`cargo build --release` для CLI;
`npm run tauri build` для GUI). AppImage/Flatpak/deb/rpm — отдельная веха позже.

## 6. Архитектура

Разделение привилегий. Один бинарь, режимы по аргументам.

```
┌───────────────────────────────────────────────────────────────┐
│                      lcm-core  (Rust lib)                       │
│   import · validate · plan · backends · audit                   │
└───────────────────────────────────────────────────────────────┘
        ▲                              ▲
        │                              │
┌───────┴────────┐            ┌────────┴─────────┐
│  CLI  `lcm`    │            │  GUI  `lcm-gui`  │   ← без привилегий (user)
│  (user)        │            │  Tauri + React   │
└───────┬────────┘            └────────┬─────────┘
        │   формируют декларативный «план» привилегированных операций
        │                              │
        └──────────────┬───────────────┘
                       │  pkexec, план как JSON по stdin
                       ▼
            ┌────────────────────────┐
            │  Helper  `lcm --helper`│   ← root, oneshot, через polkit
            │  • пишет в trust store │
            │  • запускает update-*  │
            │  • разворачивает server│
            │    cert+key, reload    │
            │  • валидирует ВСЁ сам   │
            └────────────────────────┘
```

### 6.1 Компоненты
1. **`lcm-core`** — вся доменная логика: парсинг/валидация X.509 и PKCS#12,
   определение дистрибутива, выбор backend, построение плана, аудит
   установленного. Без привилегий, без UI.
2. **CLI / GUI** — фронты. Делают user-level операции напрямую (NSS-импорт через
   `certutil`/`pk12util`, доступ к keyring). Привилегированное — только через план.
3. **Helper (`lcm --helper`)** — root, транзитный запуск через `pkexec` на
   пачку операций. Получает план JSON по stdin, **не доверяет ему**: принимает
   только whitelisted-операции над фиксированным набором директорий, сам
   определяет дистрибутив, валидирует входной X.509/ключ, пишет атомарно
   (temp + `rename`), ставит владельца/права, логирует в журнал.

### 6.2 Почему oneshot-pkexec, а не демон
`pkexec` — это и есть polkit. Транзитный helper не требует ставить системный
сервис (важно для будущей «однофайловой» поставки). UX-минус (диалог на пачку)
гасим тем, что фронт группирует все привилегированные изменения в **один план**
и зовёт helper **один раз**. Постоянный D-Bus-сервис с тонкими polkit-действиями
— опция на потом.

## 7. Привилегии и polkit
- Привилегированный код = только helper, только root, только oneshot,
  запуск `pkexec --disable-internal-agent <self> --helper`.
- **Граница доверия — внутри helper'а.** Даже скомпрометированный фронт не
  заставит записать в произвольный путь: helper знает разрешённые каталоги и
  валидирует все входные данные.
- Все привилегированные операции журналируются.
- Installed-режим с `.policy`-файлом и `auth_admin_keep` — на потом.

## 8. Модель данных

```rust
enum Item {
    CaAnchor {
        cert: X509,
        targets: Vec<Target>,        // SystemTrust, Nss(profile…)
        installed: Vec<InstalledRef>,
    },
    ClientIdentity {
        cert: X509,
        key: PrivateKeyRef,          // ключ — в защищённом хранилище
        targets: Vec<Target>,        // Nss(profile…), UserStore
        installed: Vec<InstalledRef>,
    },
    ServerCertificate {
        cert: X509,
        key: PrivateKeyRef,
        chain: Vec<X509>,
        deploy: Vec<ServiceDeploy>,  // nginx/apache/traefik/haproxy + reload
        installed: Vec<InstalledRef>,
    },
}
```

Отображаемые поля: subject/issuer, отпечаток SHA-256, серийник,
`not_before`/`not_after`, назначение (KU/EKU), статус (валиден/истекает/истёк).
`InstalledRef` фиксирует, *что и куда* реально положено, — для аудита и отката.

### 8.1 Хранение секретов
- Приватные ключи не хранятся открыто: PEM `0600` в
  `$XDG_DATA_HOME/lcm/keys/`; passphrase — в системном keyring (Secret Service).
- В памяти секреты в `zeroize`-обёртках, чистятся после использования.
- *(Потом)* импорт ключей/идентичностей прямо из зашифрованных контекстов и
  `.skb`-бандлов Secutor — без расшифровки на диск.

## 9. Backends

Два семейства, оба выбираются в рантайме.

### 9.1 TrustStoreBackend (по `/etc/os-release`)
```rust
trait TrustStoreBackend {
    fn family(&self) -> DistroFamily;
    fn anchor_dir(&self) -> &Path;
    fn anchor_filename(&self, cert: &X509) -> String;
    fn apply(&self) -> Command;                 // update-ca-certificates / update-ca-trust / trust
    fn list_installed(&self) -> Vec<InstalledRef>;
    fn remove(&self, r: &InstalledRef);
}
```
Реализации: `DebianLike`, `FedoraLike`, `ArchLike`, `SuseLike`, `AlpineLike`.
Выбор по `ID`/`ID_LIKE` с фолбэком на наличие команд.

### 9.2 NssBackend (user-level)
Обнаружение профилей: `~/.pki/nssdb` (общая), Firefox-профили
(`~/.mozilla/firefox/*/`). Импорт CA — `certutil -A`, клиентских идентичностей —
`pk12util`. Root не нужен.

### 9.3 ServiceDeployer (по `/etc/os-release` + наличие сервиса)
Знает дефолтные пути и команду reload для nginx / Apache / Traefik / haproxy:
куда класть `cert.crt`/`key.key`/`chain.crt`, владельца, права, как
проверить конфиг (`nginx -t`) и перезагрузить (`systemctl reload …`).
Сами пути и reload **захардкожены в helper'е** (фронт лишь выбирает сервис).

## 10. Сценарии

### CLI (первоклассный, для серверов/CI)
```sh
# Установить CA в системный trust store (+ браузеры)
lcm ca install corp-root.crt --system --nss

# Импорт клиентской идентичности для mTLS в браузеры
lcm client import alice.p12 --nss

# Развернуть серверный сертификат под nginx и перезагрузить
lcm server deploy site.bundle.p12 --service nginx --name example.com --reload

# Показать, что установлено, и сроки
lcm list --installed --expiring 30d

# Удалить ранее установленный CA
lcm ca remove --fingerprint a1:b2:...

# Предпросмотр без изменений (печатает план; polkit не зовётся)
lcm ca install corp-root.crt --system --dry-run
```
Пароли — флагом, из stdin или интерактивно на TTY. `--json` для машинного вывода.
Один polkit-запрос на привилегированную команду.

### GUI
Drag-and-drop файла → превью (subject/срок/отпечаток) → выбор целей
(☑ System ☑ Браузеры / сервис) → **предпросмотр плана** (точные пути и команды)
→ один polkit-диалог → применение. Вкладка «Аудит» со сроками и кнопкой удаления.

Общий принцип обоих фронтов: **никаких тихих изменений** — план показывается до
применения.

## 11. Безопасность (сводка)

| Угроза | Митигатор |
|---|---|
| Фронт скомпрометирован, просит записать произвольный путь | helper знает фиксированные каталоги, игнорирует чужие пути |
| Подсунули не-сертификат как anchor | helper парсит/валидирует X.509 перед записью |
| Утечка приватного ключа с диска | права `0600`, отдельный store, passphrase в keyring, `zeroize` |
| Повышение привилегий через helper | минимальный whitelist операций, без shell-интерполяции |
| Тихая порча trust store / конфига сервиса | обязательный предпросмотр плана + `nginx -t` перед reload + журналирование |
| Деплой серверного ключа с широкими правами | helper жёстко ставит владельца сервиса и `0600` |

## 12. Поддержка (v1)

- **Дистрибутивы:** Debian, Ubuntu, Mint, Pop!_OS (DebianLike) · Fedora, RHEL,
  CentOS Stream, Rocky, Alma (FedoraLike) · Arch, Manjaro, EndeavourOS
  (ArchLike) · openSUSE (SuseLike) · Alpine (AlpineLike).
- **Браузеры/NSS:** общая `~/.pki/nssdb`, профили Firefox.
- **Сервисы для деплоя:** nginx, Apache (httpd), Traefik (file provider), haproxy.

## 13. Связь с Secutor (на потом)
- Читать `.skb`-бандлы (certs, p12-профили, поддеревья CA) и
  контексты-SQLite Secutor напрямую → «из Secutor в систему» одним действием.
- Совместимость с его экспортом nginx/Traefik: LCM забирает сгенерированный
  Secutor'ом bundle и **доводит до системы** (ставит файлы, reload).
- Возможный общий формат метаданных, чтобы оба инструмента «видели» один объект.

## 14. Тестирование
- **Юнит:** парсинг os-release, выбор backend, валидация X.509/PKCS#12,
  генерация имён/планов.
- **Интеграция в контейнерах:** матрица образов (ubuntu/fedora/arch/opensuse/
  alpine) — реальная установка CA и проверка `openssl verify` против системного
  бандла; деплой server cert и `nginx -t`.
- **Helper-фаззинг:** мусор в JSON-плане — helper не падает и ничего не пишет.
- **NSS:** временный профиль, `certutil -L` после импорта.

## 15. Дорожная карта
- **M1 — Core+CLI skeleton ✅:** `lcm-core` (os-release, backend-трейт, Debian,
  валидация), helper-режим, CLI `ca install/remove/list`. Docker dev-loop.
- **M2 — Полный CA + NSS:** остальные дистрибутивы, NSS-импорт CA, аудит/сроки.
- **M3 — Клиентские идентичности ~:** PEM cert+key, user-store ✅; PKCS#12 и
  NSS-импорт — TODO.
- **M4 — Серверный деплой ✅:** ServiceDeployer (nginx/Apache/HAProxy), reload,
  удаление.
- **M5 — GUI ✅:** Tauri + React (Tokyo Night), 4 страницы, предпросмотр плана,
  drag-and-drop поверх `lcm-core`.
- **Упаковка ✅:** deb/rpm/AppImage для arm64 и amd64 через Makefile
  (`make packages-all`), `lcm` вшит в deb/rpm.
- **M6 — Полировка:** CLI-паритет `lcm client/server`, PKCS#12, NSS, локализация
  (en/ru), a11y, поддержка форматов Secutor.
- **M7 — Windows (см. §3.1):** WindowsBackend (ROOT-store), UAC-elevation,
  Personal store для идентичностей, `.msi`/NSIS.

## 16. Открытые вопросы
1. **Имя бинаря/пакета** — `lcm`? app-id `org.lcm.LinuxCertManager`?
2. CLI и GUI — **один бинарь** с подкомандой `lcm gui`, или **два бинаря**
   (`lcm` + `lcm-gui`) поверх общей `lcm-core`?
3. Для серверного деплоя — **только класть файлы** и звать reload, или ещё и
   **править/генерировать конфиг** сервиса (ближе к экспорту Secutor)?
4. Нужен ли **общий формат метаданных** с Secutor уже в v1, чтобы аудит видел
   происхождение объекта?
5. Минимальный набор дистрибутивов для **первого рабочего MVP** (предлагаю
   Debian/Ubuntu + Fedora)?
