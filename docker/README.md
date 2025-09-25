# Docker Setup для Luminis

Эта папка содержит все необходимые файлы для запуска Luminis в Docker контейнере.

## Быстрый старт

### 1. Настройка конфигурации

Отредактируйте файлы конфигурации:

```bash
# Основная конфигурация
nano config/config.yaml

# Секреты для Mastodon
nano secrets/mastodon.yaml
```

### 2. Настройка API ключей

**Важно:** Приложению нужен API ключ для работы с LLM провайдером (по умолчанию Gemini).

Отредактируйте файл конфигурации и добавьте ваш API ключ:

```bash
nano config/config.yaml
```

Найдите секцию `llm` и заполните `api_key`:
```yaml
llm:
  provider: Gemini
  api_key: your_actual_gemini_api_key_here
  # ... остальные настройки
```

### 3. Запуск приложения

```bash
# Первый запуск (интерактивный для авторизации в Mastodon)
docker compose up --build

# Последующие запуски (фоновый режим)
docker compose up -d
```

## Структура папки

```
docker/
├── README.md              # Этот файл
├── docker-compose.yml     # Конфигурация Docker Compose
├── Dockerfile            # Docker образ
├── .dockerignore         # Исключения для Docker
├── config/
│   └── config.yaml       # Основная конфигурация (редактируемый)
├── secrets/
│   └── mastodon.yaml     # Секреты Mastodon (редактируемый)
├── cache/                # Кэш приложения (автоматически создается)
├── logs/                 # Логи приложения (автоматически создается)
└── cargo-cache/          # Кэш Cargo для ускорения сборки (автоматически создается)
```

## Команды

| Команда | Описание |
|---------|----------|
| `docker compose up --build` | Сборка и запуск |
| `docker compose up -d` | Запуск в фоновом режиме |
| `docker compose logs -f luminis` | Просмотр логов |
| `docker compose exec luminis /bin/bash` | Подключение к контейнеру |
| `docker compose down` | Остановка |
| `docker compose down --rmi all --volumes` | Полная очистка |

## Мониторинг

### Просмотр логов
```bash
# Логи контейнера в реальном времени
docker compose logs -f luminis

# Логи из файла
tail -f logs/luminis.log
```

### Статус контейнеров
```bash
docker compose ps
```

## Первый запуск

**Важно:** При первом запуске приложение потребует интерактивной авторизации в Mastodon для получения токена доступа. Запускайте контейнер в интерактивном режиме:

```bash
docker compose up --build
```

После успешной авторизации токен будет сохранен в `secrets/mastodon.yaml` и последующие запуски будут работать автоматически.

## Устранение проблем

### Проблемы с авторизацией Mastodon
```bash
# Если авторизация не прошла, удалите сохраненный токен и попробуйте снова
rm secrets/mastodon.yaml
docker compose up --build
```

### Проблемы с правами доступа
```bash
# Исправление прав на директории
sudo chown -R $USER:$USER cache logs secrets config cargo-cache
```

### Очистка кэша Cargo
```bash
# Если возникают проблемы со сборкой, можно очистить кэш Cargo
rm -rf cargo-cache/*
docker compose build --no-cache
```

### Очистка Docker
```bash
# Очистка неиспользуемых ресурсов
docker system prune -f

# Полная очистка
docker system prune -a -f --volumes
```

## Безопасность

- Приложение запускается под непривилегированным пользователем `luminis`
- Секреты монтируются как volume'ы (не встраиваются в образ)
- Конфигурационные файлы можно редактировать без пересборки
