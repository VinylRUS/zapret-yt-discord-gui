# Настройка релизов и автообновления — Zapret GUI

Это пошаговая инструкция: как выпустить первую версию `v1.0.0` в GitHub
Releases и включить рабочее автообновление приложения (in-app updater).

Все шаги делаются **один раз**. После них релиз новой версии сводится к одной
команде `git tag v1.x.0 && git push origin v1.x.0`.

---

## 0. Что вообще происходит

Когда вы пушите тег `v*`, CI (`.github/workflows/build.yml`, job `release`):

1. Собирает фронтенд и Rust-часть.
2. Подставляет в `tauri.conf.json` реальный адрес репозитория и публичный ключ
   (через секреты — конф править руками не нужно).
3. Собирает NSIS-инсталлер `.exe` и `.msi`.
4. **Подписывает** их вашим приватным ключом → рядом появляются `.sig`-файлы.
5. Через `tauri-apps/tauri-action` создаёт **не-draft, не-prerelease** релиз и
   кладёт туда: `.exe`, `.msi`, `.sig`, и **`latest.json`** — манифест
   автообновления.

Приложение при запуске (вкладка «Обновления») дёргает эндпоинт

```
https://github.com/<owner>/<repo>/releases/latest/download/latest.json
```

GitHub автоматически редиректит `/releases/latest/...` на самый свежий
**стабильный** релиз. Из `latest.json` приложение узнаёт версию, качает
`.exe`, сверяет подпись публичным ключом (зашит в бинарник), ставит и
перезапускается.

> Nightly-сборки (push в `main`) — это **prerelease** с тегом `nightly`,
> автообновление на них не ведёт. Они нужны только для теста свежего кода.

---

## 1. Предварительные требования

- Репозиторий уже на GitHub, ветка `main` существует.
- GitHub Actions включён (Settings → Actions → General → Allow all actions).
- На машине разработчика стоит Node 20+ и Rust stable
  (`rustup default stable`).
- В репо лежит файл `package-lock.json`? Если нет — CI использует `npm install`
  и сгенерирует его при первом прогоне. **Зак Commite его** после первого
  успешного прогона, чтобы сборки были воспроизводимы.

---

## 2. Генерация ключевой пары подписи (один раз)

Это **ваши** ключи. Приватный ключ — секрет репозитория, публичный —
вставляется в конфиг через секрет и зашивается в бинарник.

На машине разработчика, в корне проекта:

```bash
npx @tauri-apps/cli signer generate -w ~/.tauri/zapret-gui.key
```

Утилита спросит пароль (можно пустой, но лучше задать). Вывод будет примерно
таким:

```
A new key pair was generated.
Your private key was saved to /home/<you>/.tauri/zapret-gui.key.
Your public key was saved to /home/<you>/.tauri/zapret-gui.key.pub.

Public Key:
dW50cnVzdGVkIGNvbW1lbnQ6IG1pbnNpZ25hdHVyZSBmcm9tIHRhdXJpLmFwcHMK...

Private Key:
UNTRUSTED.ONLY FOR LOCAL DEVELOPMENT. dW50cnVzdGVkIGNvbW1lbnQ6...
```

**Сохраните три значения:**
- Публичный ключ (строка `dW50...`).
- Приватный ключ (строка `UNTRUSTED...` — целиком, с префиксом).
- Пароль, который вы задали.

> Файл `~/.tauri/zapret-gui.key` — приватный. Никогда не коммитьте его и не
> вставляйте в код. Если потеряете — автообновление старых версий сломается,
> придётся выпускать новую мажорную версию с новым ключом.

---

## 3. Добавление секретов в репозиторий

GitHub → ваш репозиторий → **Settings → Secrets and variables → Actions →
New repository secret**. Добавьте три секрета:

| Имя секрета | Значение |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | приватный ключ (строка `UNTRUSTED...` целиком) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | пароль от ключа (пустая строка, если задали пустой) |
| `TAURI_UPDATER_PUBKEY` | публичный ключ (строка `dW50...`) |

Все три используются в job'е `release`. `TAURI_UPDATER_PUBKEY` — это не
настоящий «секрет» (публичный ключ нечувствителен), но для единообразия
положим его туда; при желании можно использовать repository Variables вместо
Secrets.

---

## 4. Конфиг уже настроен — проверьте

В `src-tauri/tauri.conf.json` секция `plugins.updater` содержит плейсхолдеры:

```json
"plugins": {
  "updater": {
    "endpoints": [
      "https://github.com/__REPO__/releases/latest/download/latest.json"
    ],
    "pubkey": "__PUBKEY__"
  }
}
```

`__REPO__` и `__PUBKEY__` подставляются CI на шаге «Inject updater config».
**Править руками ничего не надо.**

Разрешения для фронтенда уже выданы в
`src-tauri/capabilities/default.json`:
`updater:allow-check`, `updater:allow-download-and-install`,
`process:allow-restart`.

---

## 5. Первый релиз — v1.0.0

Убедитесь, что версия в `tauri.conf.json` = `"version": "1.0.0"` (она уже
такая). Если хотите другую — поменяйте и там, и в теге.

```bash
git add -A
git commit -m "release: v1.0.0 + auto-update"
git push origin main

git tag -a v1.0.0 -m "Zapret GUI v1.0.0"
git push origin v1.0.0
```

Пуш тега запускает job `release`. Следите за прогрессом:
**Actions → "Build & Release (Windows)" → release**. Сборка идёт ~6–10 минут
(Rust + Tauri с нуля; с кэшем быстрее).

### Что должно получиться в релизе

GitHub → Releases → **Zapret GUI v1.0.0** (помечен `Latest`), с ассетами:

- `Zapret GUI_1.0.0_x64-setup.exe` — NSIS-инсталлер (то, что ставит юзер).
- `Zapret GUI_1.0.0_x64_en-US.msi` — MSI-инсталлер.
- `Zapret GUI_1.0.0_x64-setup.exe.sig` — подпись NSIS.
- `...msi.sig` — подпись MSI.
- `latest.json` — манифест автообновления.

Если `latest.json` **отсутствует** — значит секрет `TAURI_SIGNING_PRIVATE_KEY`
пустой или кривой, и tauri-action не подписал бандлы. Перепроверьте секрет из
шага 3 и перезапустите job.

### Как выглядит `latest.json`

```json
{
  "version": "1.0.0",
  "notes": "Zapret GUI v1.0.0 — Windows installer...",
  "pub_date": "2026-11-21T20:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6...",
      "url": "https://github.com/<owner>/<repo>/releases/download/v1.0.0/Zapret%20GUI_1.0.0_x64-setup.exe"
    }
  }
}
```

---

## 6. Проверка автообновления end-to-end

1. Скачайте `Zapret GUI_1.0.0_x64-setup.exe` из релиза, установите, запустите.
   Убедитесь, что окно открывается (если падает — пришлите
   `%APPDATA%\zapret-gui\logs\panic.log`, panic-hook уже пишет туда).
2. **Поднимите версию в `tauri.conf.json`** до `1.0.1` и закоммитьте в `main`
   (можно без тега — nightly). Либо сразу тег `v1.0.1`.
3. Дождитесь, пока CI соберёт новый релиз с `latest.json` (version `1.0.1`).
4. В установленном v1.0.0 откройте вкладку **«Обновления»** — должен появиться
   баннер «Доступно обновление GUI: 1.0.1» и кнопка **«Установить обновление»**.
5. Нажмите → прогресс-бар → приложение скачивает `.exe`, сверяет подпись,
   ставит (NSIS-режим: тихая замена) и перезапускается через
   `@tauri-apps/plugin-process` `relaunch()`.
6. После перезапуска версия должна быть 1.0.1.

### Ручная проверка эндпоинта

Откройте в браузере:

```
https://github.com/<owner>/<repo>/releases/latest/download/latest.json
```

Должен либо скачаться `latest.json` (если стабильный релиз есть), либо 404
(если ещё нет ни одного стабильного релиза — это нормально для самого первого
прогона, пока `v1.0.0` не стал `Latest`).

---

## 7. Дальнейшие релизы

```bash
# bumped version in tauri.conf.json to 1.1.0
git commit -am "release: v1.1.0"
git tag -a v1.1.0 -m "v1.1.0"
git push origin v1.1.0
```

CI пересоберёт, пере подпишет, перезапишет `latest.json` в новом релизе, и
GitHub сам переключит `/releases/latest/` на v1.1.0. Все установленные копии
на следующем запуске увидят апдейт.

---

## 8. Типичные проблемы

**`latest.json` не появился в релизе.**
Секрет `TAURI_SIGNING_PRIVATE_KEY` не задан или кривой. tauri-action
генерирует манифест только если бандлы подписаны. Перепроверьте шаг 3.

**В приложении баннер не появляется.**
- Публичный ключ не подставился: проверьте секрет `TAURI_UPDATER_PUBKEY` и
  лог шага «Inject updater config» (должно быть `pubkey set -> True`).
- Нет стабильного релиза: `/releases/latest/` ведёт на prerelease/ничего.
  Должен быть хотя бы один `v*`-релиз с `prerelease: false`.
- Версия в `latest.json` ≤ установленной — апдейта нет (это правильно).

**`signature does not match` / `invalid signature`.**
Публичный ключ в `tauri.conf.json` (через `TAURI_UPDATER_PUBKEY`) не
соответствует приватному ключу (`TAURI_SIGNING_PRIVATE_KEY`). Перегенерируйте
пару (шаг 2) и обновите все три секрета одновременно.

**Приложение падает на старте.**
Panic-hook пишет `%APPDATA%\zapret-gui\logs\panic.log` и показывает MessageBox
с причиной. Пришлите файл.

**CI: `npm ci` падает на отсутствие `package-lock.json`.**
Зак Commite `package-lock.json` (он создаётся при первом `npm install` в CI) —
либо переключите обратно на `npm install` в workflow.

---

## 9. Что НЕ делает автообновление

- Не обновляет бинарники zapret (`winws.exe` и списки) — это отдельная кнопка
  «Update zapret binaries» на той же вкладке, она качает с upstream-репо
  zapret.
- Не обновляет сам WinDivert-драйвер.
- Auto-update касается только GUI-приложения (Tauri-бандла).
