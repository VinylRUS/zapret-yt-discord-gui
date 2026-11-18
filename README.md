# Zapret GUI

Лёгкая (Tauri 2 / Rust / Svelte) GUI-обёртка над [zapret](https://github.com/bol-van/zapret)
для обхода DPI-блокировок Discord, YouTube и других сервисов на Windows. Функциональность
перенесена из [Flowseal/zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube)
в один компактный системный трей-апп.

## Возможности

| # | Возможность | Где |
|---|---|---|
| 1 | Установка Windows-сервиса | Status → «Установить сервис» |
| 2 | Удаление Windows-сервиса (+ чистка WinDivert) | Status → «Удалить сервис» |
| 3 | Realtime-проверка работы сервиса | Top bar (зелёная/красная точка + health-пинг) |
| 4 | Game Filter / IPSet Filter | Вкладка «Фильтры» |
| 5 | Автопроверка апдейтов с GitHub | Settings (toggle) + вкладка «Обновления» |
| 6 | Диагностика и подбор стратегий | Вкладки «Диагностика» и «Стратегии → Подобрать» |

## Стек

- **Backend:** Rust, Tauri 2 (`windows-service`, `windows`, `reqwest`, `tokio`, `zip`)
- **Frontend:** Svelte 5 + Vite + Tailwind CSS 3, TypeScript
- **UI:** в стиле Cloudflare One Client — узкий sidebar + top bar с состоянием + карточки
- **Бандл:** MSI + NSIS, WebView2 bootstrapper, размер ~10–15 МБ

В трей-режиме приложение потребляет ~40–70 МБ ОЗУ (системный WebView2).

## Структура

```
zapret-gui/
├── .github/workflows/build.yml   # CI: windows-only сборка
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/default.json
│   ├── icons/
│   └── src/
│       ├── main.rs               # точка входа (windows_subsystem)
│       ├── lib.rs                # Tauri setup + IPC-команды
│       ├── config.rs             # config.toml, пути к bin/lists
│       ├── service.rs            # Windows SCM install/remove/status
│       ├── process.rs            # ad-hoc winws.exe запуск/стоп
│       ├── strategy.rs           # пресеты + автоматический подбор
│       ├── filters.rs            # Game/IPSet семантика
│       ├── updater.rs            # GitHub releases + обновление bin/lists/hosts
│       ├── monitor.rs            # realtime статус (events)
│       └── diagnostics.rs        # чек-лист диагностики
├── src/                          # Svelte frontend
│   ├── App.svelte
│   ├── main.ts
│   ├── app.css
│   └── lib/
│       ├── api.ts                # типизированные invoke-обёртки
│       ├── stores.ts             # svelte stores + event listeners
│       ├── components/           # Sidebar, Topbar
│       └── pages/                # Status, Strategies, Filters, Updates, Diagnostics, Settings
├── tests/                        # (placeholder)
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── README.md
```

## Разработка локально (Windows)

```powershell
# 1. Установить Rust (MSVC toolchain) и Node.js 20+
# 2. Установить зависимости
npm install

# 3. Запустить в dev-режиме (поднимет Vite + Tauri окно)
npm run tauri dev

# 4. Сборка релизного инсталлятора
npm run tauri build
```

Готовые артефакты появятся в
`src-tauri/target/release/bundle/{msi,nsis}/`.

## CI

`.github/workflows/build.yml` запускается на `windows-latest`:

1. `npm ci`
2. `cargo test --locked` (Rust unit + integration тесты)
3. `npm run build` (frontend)
4. `tauri-action` — сборка MSI и NSIS для `x86_64-pc-windows-msvc`
5. Загрузка артефактов (`zapret-gui-windows-msi`, `zapret-gui-windows-nsis`)

Для релиза по тегу `v*` создаётся draft release с апдейтером `latest.json`.

### Подпись апдейтера

Чтобы плагин updater принимал подписанные апдейты:

```powershell
npm run tauri signer generate -w zapret-gui.key
```

Задайте секреты репозитория `TAURI_SIGNING_PRIVATE_KEY` и
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, а публичный ключ вставьте в
`src-tauri/tauri.conf.json`, добавив блок `plugins.updater`:

```json
"plugins": {
  "updater": {
    "active": true,
    "endpoints": [
      "https://github.com/<your-org>/zapret-gui/releases/latest/download/latest.json"
    ],
    "dialog": true,
    "pubkey": "<PASTE_PUBKEY_HERE>"
  }
}
```

Без этого блока вкладка «Обновления» просто не покажет self-update
(апдейт самих zapret-бинарников работает и без него).

## Runtime-данные

Приложение хранит все бинарники и конфиги в
`%APPDATA%\zapret-gui\` (аналог `%LOCALAPPDATA%`):

```
%APPDATA%\zapret-gui\
├── bin\          # winws.exe, WinDivert*.sys/.dll, fake_sni*
├── lists\        # list-general-user.txt, ipset-all.txt, exclude
├── logs\
├── config.toml   # настройки GUI
└── presets.json  # редактируемый список стратегий
```

При первом запуске `bin\` пуст — нажмите на вкладке «Обновления» →
**Update zapret binaries**: приложение скачает последний релиз
`Flowseal/zapret-discord-youtube` и распакует `bin/` и `lists/`.

## UAC

Управление сервисом и WinDivert требует прав администратора. Если приложение
запущено без elevation, вверху появится жёлтое предупреждение; действия с
сервисом вернут ошибку. Перезапустите приложение от имени администратора.

## Безопасность / AV

`winws.exe` и `WinDivert` могут вызывать срабатывание антивирусов — это
известное поведение оригинального zapret и не является вирусом. Добавьте
`%APPDATA%\zapret-gui\bin\` в исключения AV при необходимости.

## Лицензия

MIT — см. [LICENSE](LICENSE). Сам zapret распространяется под отдельной
лицензией (см. upstream-репозитории).
