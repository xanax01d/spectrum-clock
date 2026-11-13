<div align="center">

# spectrum-clock

**Терминальный аудиоспектроанализатор, часы и синхронизированный плеер текста песен на Rust**

[![Rust](https://img.shields.io/badge/Rust-2024%20Edition-orange?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey?style=for-the-badge)](#требования)

[English](README.md) · **Русский**

</div>

---

```text
  ✦         ·              •                  ✦                 ·
      ██  ▄█▀▄  ▄█▀▄   ▀   █▄ ▄█  ▄█▀▄       ·         •
      ██  █  █  █  █   ▄   █ ▀ █  █  █
      ██  ▀█▄▀  ▀█▄▀   ▀   █   █  ▀█▄▀    2026-11-13 • Friday • +8°C Clear
                 ▶ Artist — Track Title
             ━━━━━━━━━━━━●────────────────────── 01:42 / 03:18
                     ♪ Текущая строка текста песни
      ▁▂▃▅▇█▇▅▃▂▁▂▄▆█▆▄▂▁▂▃▅▇█▇▅▃▂▁▂▄▆█▆▄▂▁▂▃▅▇█▇▅▃▂▁▂▄▆█▆▄▂▁
   Cover | Bars (Full) | Bg:Stars | 1.4x | [AGC] | WASAPI Loopback
```

Написан на [`ratatui`](https://ratatui.rs/), [`rustfft`](https://crates.io/crates/rustfft) и [`cpal`](https://crates.io/crates/cpal). Основная платформа — **Windows Terminal** (захват системного звука через WASAPI loopback и интеграция с Windows Media Session / SMTC), с кроссплатформенным fallback через `cpal`.

---

## Возможности

| Категория | Что внутри |
| --- | --- |
| **Визуализация (`M`)** | `Bars`, `Mirror`, `Waveform`, `Waterfall`, `Circle`, `Horizon`, `Lissajous` |
| **Темы (`T`)** | `Cover` (адаптивная палитра из обложки трека), `Cyberpunk`, `Synthwave`, `Neon`, `Rainbow`, `Matrix`, `Fire`, `Ocean`, `System` + свои темы в `themes.conf` |
| **Часы и таймеры (`F`, `W`)** | 4 шрифта (`Block`, `Neon`, `Segment`, `Dots`) · Часы / Помодоро / Секундомер · Будильник (`N`) |
| **Фоновые эффекты (`B`)** | `Stars`, `Matrix`, `Aurora`, `Pulse`, `Fire`, `Off` |
| **Текст песен (`Y`)** | Цикл `1 строка` → `3 строки караоке` → `Полноэкранный Lyrics Mode` через [`lrclib.net`](https://lrclib.net) (`.lrc` + `plainLyrics` fallback) |
| **Now Playing** | Трек и прогресс-бар из Windows SMTC, пиксельная обложка 12×6 (`I`), управление плеером (`Space`, `←`, `→`) |
| **Погода и Idle Sleep** | **Погода (`K`)** рядом с датой (`wttr.in`, раз в 30 мин) · **Idle Sleep (`L`)** скрывает спектр через 15с тишины |
| **Аудио и интерфейс** | 2048-точечный FFT (`Full` / `Bass` / `Mids`), автоусиление (`A`), VU-метры L/R, BPM, отражение (`R`), защита OLED (`O`), компактный режим (`C`), Zen (`Z`) |

---

## Быстрый старт

### Требования

- **Rust** (Edition 2024, например Rust 1.85+)
- Терминал с поддержкой **TrueColor (24-bit RGB)** и Unicode (рекомендуется Windows Terminal)

### Сборка и запуск

```bash
git clone https://github.com/xanax01d/spectrum-clock.git
cd spectrum-clock
cargo run --release
```

<details>
<summary>Проверка тестов и линтера</summary>

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

</details>

---

## Управление и горячие клавиши

| Клавиша | Действие | Клавиша | Действие |
| :---: | --- | :---: | --- |
| `T` | Переключить цветовую тему | `Space` | Пауза / Воспроизведение |
| `M` | Переключить режим спектра | `←` / `→` | Предыдущий / Следующий трек |
| `F` | Переключить шрифт часов | `+` / `-` · `↑` / `↓` | Изменить усиление |
| `B` | Переключить фоновый эффект | `A` | Автоусиление (AGC) |
| `Y` | Режим текста (`1L` / `3L` / `Full`) | `Enter` | Сбросить Помодоро / Секундомер |
| `W` | Часы / Помодоро / Секундомер | `X` | Party-режим (автосмена) |
| `I` | Показ обложки альбома | `S` | Показ секунд |
| `R` | Водное отражение | `D` | Плотность полос |
| `E` | Диапазон частот | `N` | Отключить активный будильник |
| `O` | Защита OLED (сдвиг пикселей) | `P` | Заморозить спектр |
| `C` | Компактный режим | `K` | Показ погоды |
| `Z` | Zen-режим (автоскрытие панели) | `L` | Idle Sleep (сон в тишине) |
| `?` / `H` | Справка | `Q` / `Esc` | Выход |

---

## Конфигурация

Настройки автоматически сохраняются в `config.conf`:

- **Windows**: `%APPDATA%\spectrum-clock\config.conf`
- **Linux / macOS**: `~/.config/spectrum-clock/config.conf`

Подробное описание всех ключей, формата будильника (`HH:MM|Message`), `weather_city` и кастомных тем `themes.conf` — в **[docs/CONFIGURATION.md](docs/CONFIGURATION.md)**.

---

<details>
<summary>Архитектура проекта</summary>

| Модуль | Назначение |
| --- | --- |
| `src/main.rs` | Главный цикл, обработка клавиш, координация модулей и конфиг |
| `src/audio.rs` | Захват аудио (WASAPI loopback на Windows и `cpal`) |
| `src/fft.rs` | 2048-точечный FFT-анализатор, AGC, bass pulse и оценка BPM |
| `src/ui.rs` | Разметка `ratatui`, 7 режимов спектра, Lyrics Mode, VU-метры, оверлеи |
| `src/clock.rs` | 4 шрифта часов, таймер Помодоро и секундомер |
| `src/theme.rs` | Встроенные палитры, системный акцент и парсер `themes.conf` |
| `src/particles.rs` | Движок анимированных фоновых эффектов |
| `src/now_playing.rs` | Интеграция с Windows SMTC и палитра из обложки трека |
| `src/lyrics.rs` | Фоновая загрузка `.lrc` / `plainLyrics` с `lrclib.net` |
| `src/weather.rs` | Фоновый опрос погоды с `wttr.in` |
| `src/config.rs` | Сериализация и парсинг `config.conf` и `themes.conf` |

</details>

## Лицензия

Проект распространяется под лицензией **[MIT License](LICENSE)**.
