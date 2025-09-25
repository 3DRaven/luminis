# Luminis

Mastodon/Telegram бот и офлайн-генератор кратких сводок по проектам НПА.

## Что это

- Находит новые элементы (например https://regulation.gov.ru/projects/160590, при ошибках — RSS fallback)
- Извлекает текст из документов, суммаризирует через LLM
- Публикует в выбранные каналы или выводит локально (консоль/файл)
- Работает периодически (фон) или однократно (single-shot)

Диаграмма последовательности находится в `docs/main.md` и отражает текущую архитектуру подсистем.

## 🐳 Docker Setup (Рекомендуемый способ)

Проект настроен для работы в Docker контейнерах, что позволяет запускать его на любой машине без установки внешних зависимостей.

### Быстрый старт

#### 1. Клонирование репозитория
```bash
git clone <repository-url>
cd luminis
```

#### 2. Настройка и запуск

Перейдите в папку `docker` и запустите приложение:

```bash
cd docker
docker compose up --build
```

Это автоматически создаст необходимые директории и запустит приложение.

#### 3. Редактирование конфигурации

Отредактируйте файлы в папке `docker`:
- `config/config.yaml` - основная конфигурация (включая API ключи)
- `secrets/mastodon.yaml` - секреты для Mastodon

**Важно:** Добавьте ваш API ключ в `config/config.yaml` в секции `llm`:
```yaml
llm:
  provider: Gemini
  api_key: your_actual_gemini_api_key_here
```

#### 4. Запуск приложения

**Важно:** При первом запуске приложение потребует интерактивной авторизации в Mastodon для получения токена доступа. Запускайте контейнер в интерактивном режиме:

```bash
cd docker
docker compose up
```

Или напрямую через Docker Compose:
```bash
cd docker
docker compose up --build
```

После успешной авторизации токен будет сохранен в `secrets/mastodon.yaml` и последующие запуски будут работать автоматически.

Для фонового запуска (после первой авторизации):
```bash
cd docker
docker compose up -d
```

### Доступные команды

| Команда | Описание |
|---------|----------|
| `cd docker && docker compose up --build` | Сборка и запуск приложения |
| `cd docker && docker compose run --rm -it luminis luminis --config /app/config/config.yaml` | Интерактивный запуск для авторизации в Mastodon |
| `cd docker && docker compose build` | Только сборка Docker образа |
| `cd docker && docker compose up` | Запуск без пересборки |
| `cd docker && docker compose up -d` | Запуск в фоновом режиме |
| `cd docker && docker compose logs -f luminis` | Просмотр логов |
| `cd docker && docker compose exec luminis /bin/bash` | Подключение к контейнеру |
| `cd docker && docker compose down` | Остановка контейнера |
| `cd docker && docker compose down --rmi all --volumes` | Полная очистка |

### Структура Docker

#### Папка docker/
```
docker/
├── README.md              # Инструкции по Docker
├── docker-compose.yml    # Конфигурация Docker Compose
├── Dockerfile            # Docker образ
├── .dockerignore         # Исключения для Docker
├── config/
│   └── config.yaml       # Основная конфигурация (включая API ключи)
├── secrets/
│   └── mastodon.yaml     # Секреты Mastodon (редактируемый)
├── cache/                # Кэш приложения (автоматически создается)
├── logs/                 # Логи приложения (автоматически создается)
└── cargo-cache/          # Кэш Cargo для ускорения сборки (автоматически создается)
```

#### Образы
- **luminis** - production образ приложения

#### Volumes
- `docker/cache/` - кэш приложения (персистентный)
- `docker/logs/` - логи приложения (ежедневная ротация)
- `docker/secrets/` - конфигурационные файлы с секретами (редактируемые)
- `docker/config/config.yaml` - основная конфигурация (редактируемая)
- `docker/cargo-cache/` - кэш Cargo для ускорения повторных сборок (персистентный)

#### Сети
- `luminis-network` - внутренняя сеть для связи между сервисами

### Мониторинг

#### Просмотр логов
```bash
# Логи контейнера в реальном времени
cd docker && docker compose logs -f luminis

# Логи из файла
tail -f logs/luminis.log
```

#### Логирование в файлы
Приложение автоматически записывает логи в файлы в директории `logs/`:
- `logs/luminis.log` - логи production версии

Логи ротируются ежедневно с суффиксом даты (например, `luminis.log.2024-01-15`).

**Настройка логирования:** По умолчанию в Docker контейнере логирование настроено через параметр `--log-file /app/logs/luminis.log`. Для локального запуска можно использовать:
```bash
cargo run -- --log-file ./logs/luminis.log
```

#### Статус контейнеров
```bash
cd docker && docker compose ps
```

### Безопасность

- Приложение запускается под непривилегированным пользователем `luminis`
- Секреты монтируются как volume'ы (не встраиваются в образ)
- Конфигурационные файлы можно редактировать без пересборки

### Очистка

#### Остановка контейнеров
```bash
cd docker && docker compose down
```

#### Полная очистка
```bash
cd docker && docker compose down --rmi all --volumes --remove-orphans
docker system prune -f
```

Это удалит:
- Все контейнеры
- Все образы
- Все volume'ы
- Кэш Docker

### Требования

- **Docker**: 20.10+
- **Docker Compose**: 2.0+
- **Диск**: минимум 2GB свободного места

## 🔧 Локальная установка (Альтернативный способ)

### Установка и запуск

1. https://www.rust-lang.org/tools/install
2. git clone git@github.com:3DRaven/luminis.git
3. cd ./luminis
4. cargo build --release
5. Отредактируйте `config.yaml` под ваши нужды (см. примеры ниже)
6. (Опционально) создайте `.env` с API-ключами LLM провайдера (например, `GROQ_API_KEY=...`)
7. Запуск: `cargo run --release`

По умолчанию приложение читает конфиг из `config.yaml` в корне проекта.

## Режимы использования (без публичной публикации и с ней)

Можно запускать полностью локально — без Mastodon/Telegram — и получать результат только в консоли и/или записывать в файл. Каналы включаются/выключаются в `config.yaml`.

### A) Только консоль
```yaml
output:
  console_enabled: true
  console_max_chars: 10000
  file_enabled: false
```

### B) Только файл (без сети)
```yaml
output:
  file_enabled: true
  file_path: "./post.txt"
  file_append: true
  file_max_chars: 20000
  console_enabled: false
```

### C) Консоль + файл (оба локально)
```yaml
output:
  console_enabled: true
  file_enabled: true
  file_path: "./post.txt"
  file_append: true
```

### D) Публикация в Mastodon
```yaml
mastodon:
  base_url: "https://mastodon.social"
  access_token: ""          # можно оставить пустым
  enabled: true
  login_cli: true            # при пустом токене предложит интерактивный вход и сохранит его в ./secrets/mastodon.yaml
  visibility: "unlisted"    # public|unlisted|private|direct
  language: "ru"
  spoiler_text: "Новости"
  sensitive: false
```

**КРИТИЧЕСКИ ВАЖНО:** Если Mastodon включен как канал публикации (`enabled: true`), приложение требует успешной авторизации. Если `access_token` пуст и `login_cli: true`, при первом запуске пройдёт интерактивная авторизация; токен сохранится в `./secrets/mastodon.yaml`. Если авторизация не удалась или `login_cli: false` при отсутствии токена, приложение завершится с ошибкой.

**Примечание для Docker:** При первом запуске в контейнере обязательно используйте интерактивный режим для авторизации в Mastodon:

```bash
cd docker
docker compose run --rm -it luminis luminis --config /app/config/config.yaml
```

После авторизации токен сохранится в `docker/secrets/mastodon.yaml`, и в дальнейшем можно использовать обычный режим:

```bash
docker compose up
```

### E) Публикация в Telegram
```yaml
telegram:
  api_base_url: "https://api.telegram.org"
  bot_token: "000000:xxxxx"
  target_chat_id: 123456789
  enabled: true
  max_chars: 4096
```

### F) Смешанные режимы
Вы можете одновременно включить любые каналы: `console`, `file`, `mastodon`, `telegram`. Сервис сам пропускает уже опубликованные каналы благодаря кэшу.

**Важно о лимитах:** Каналы (`telegram.max_chars`, `mastodon.max_chars`, `console_max_chars`, `file_max_chars`) передаются в промпт модели как мягкие ограничения. Глобальный `run.post_max_chars` — это жесткий лимит безопасности: итоговый пост всегда обрезается до этого размера независимо от того, что вернула модель.

## Минимальный конфиг (каркас)
```yaml
llm:
  provider: "Groq"          # см. провайдеры ниже
  model: "mixtral-8x7b"     # имя модели для LLM
  request_timeout_secs: 60

crawler:
  interval_seconds: 300
  request_timeout_secs: 30
  poll_delay_secs: 0         # можно увеличить, чтобы притормозить LLM-вызовы
  max_retry_attempts: 3      # 0 = бесконечно
  npalist:
    enabled: true
    url: "https://regulation.gov.ru/..." # шаблон источника
    limit: 10
    regex: "projectId=(?P<id>\\d+)"    # опционально, извлечение project_id
  rss:
    enabled: true
    url: "https://example.com/feed.xml"
    regex: ".*"                          # для fallback

output:
  console_enabled: true
  file_enabled: false

run:
  post_template: "{{ title }}\n{{ summary }}\n{{ url }}"  # Tera-шаблон итогового поста (обязателен)
  post_max_chars: 2048            # жесткий лимит итогового поста (обрежется с троеточием)
  input_sample_percent: 0.05      # доля начала текста документа в промпте (0..1)
  summarization_timeout_secs: 120
  cache_dir: "./cache"
  max_posts_per_run: 3            # лимит публикаций за запуск (опционально)
```

## Режимы запуска
- Фоновый (по интервалу): `crawler.npalist.interval_seconds` определяет, как часто краулер пытается получить новые элементы. При неудаче используется RSS fallback с ретраями.
- Однократный (single-shot): установите ограничение `run.max_posts_per_run` и дайте приложению завершиться после достижения лимита. В этом случае подсистема Worker завершит работу и запросит shutdown остальных подсистем.

## Провайдеры LLM и ключи
LLM вызывается через `ai-lib`. Задаётся `llm.provider` (например, `Groq`, `OpenAI`, `Gemini`, `Anthropic`, `Mistral`, `TogetherAI`, `Cohere`, и др.). Ключ можно задать через переменную окружения `<PROVIDER>_API_KEY` (например, `GROQ_API_KEY`) или в `llm.api_key`. Поддерживаются также `llm.base_url`, `llm.proxy`, `llm.request_timeout_secs` и пр.

## Кэш
Все артефакты сохраняются поэтапно в `run.cache_dir`:
- исходные данные/markdown
- суммаризации (общая и канал-специфичные)
- итоговые посты (по каналам)
- статус опубликованных каналов

## Примечания
- Поля `run.post_template` и (при публикации) корректные настройки каналов обязательны.
- Mastodon: если `login_cli: true` и нет токена — при первом запуске потребуется интерактивное подтверждение, после чего токен сохраняется в `./secrets/mastodon.yaml`.
- Telegram: требуется корректный `bot_token` и `target_chat_id`.

## 🔧 Устранение проблем

### Проблемы с авторизацией Mastodon
```bash
# Если авторизация не прошла, удалите сохраненный токен и попробуйте снова
rm secrets/mastodon.yaml
cd docker && docker compose up --build
```

**Важно:** Если Mastodon включен в конфигурации (`enabled: true`), но авторизация не удалась, приложение завершится с ошибкой. Это сделано намеренно для предотвращения работы с неработающим каналом публикации. Убедитесь, что:
- Указан корректный `base_url` для вашего инстанса Mastodon
- Установлен `login_cli: true` для интерактивной авторизации
- Или указан валидный `access_token` в конфигурации

### Проблемы с правами доступа
```bash
# Исправление прав на директории
sudo chown -R $USER:$USER cache logs secrets
```

### Проблемы с портами
```bash
# Проверка занятых портов
netstat -tulpn | grep :8080
```

### Очистка Docker
```bash
# Очистка неиспользуемых ресурсов
docker system prune -f

# Полная очистка
docker system prune -a -f --volumes
```