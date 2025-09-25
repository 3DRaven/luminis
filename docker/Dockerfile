# Multi-stage build для Rust приложения
FROM rust:1.90-slim as builder

# Установка системных зависимостей
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    mc \
    && rm -rf /var/lib/apt/lists/*

# Создание рабочей директории
WORKDIR /app

# Копирование файлов зависимостей
COPY Cargo.toml Cargo.lock ./

# Копирование исходного кода
COPY src ./src
COPY config.yaml.example ./config.yaml.example
COPY secrets/mastodon.yaml.example ./secrets/mastodon.yaml.example

# Создание директории для секретов
RUN mkdir -p secrets

# Сборка приложения
RUN cargo build --release

# Финальный образ
FROM debian:bookworm-slim

# Установка runtime зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Создание пользователя для безопасности
RUN useradd -r -s /bin/false luminis

# Создание рабочих директорий
RUN mkdir -p /app/cache /app/secrets /app/logs && \
    chown -R luminis:luminis /app && \
    chmod 755 /app/logs

# Копирование бинарного файла
COPY --from=builder /app/target/release/luminis /usr/local/bin/luminis

# Копирование конфигурационных файлов
COPY --from=builder /app/config.yaml.example /app/config.yaml.example
COPY --from=builder /app/secrets/mastodon.yaml.example /app/secrets/mastodon.yaml.example

# Переключение на пользователя luminis
USER luminis

# Рабочая директория
WORKDIR /app

# Открытие портов (если приложение их использует)
EXPOSE 8080

# Команда по умолчанию
CMD ["luminis", "--config", "/app/config/config.yaml"]
