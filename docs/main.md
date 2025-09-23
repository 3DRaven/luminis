# Luminis - Архитектура приложения

## Диаграмма последовательности основного потока

```mermaid
sequenceDiagram
    participant App as Application
    participant Lib as lib.rs
    participant NPA as NpaListCrawlerSubsystem
    participant RSS as RssCrawler
    participant WorkerSub as WorkerSubsystem
    participant Worker as Worker
    participant ChannelMgr as ChannelManager
    participant MarkdownFetcher as DocxMarkdownFetcher
    participant Summarizer as Summarizer
    participant CacheManager as CacheManager
    participant Publishers as Publishers
    participant Mastodon as Mastodon
    participant Telegram as Telegram
    participant Console as Console
    participant File as File

    Note over App: Инициализация приложения
    App->>Lib: run_with_config_path()
    Lib->>Lib: load_config()
    Lib->>Lib: init_logging()
    Lib->>Lib: init_chat_api(LocalChatApi)
    Lib->>Lib: init_summarizer()
    Lib->>Lib: init_telegram_api(Optional)
    Lib->>Lib: init_cache_manager()
    Lib->>Lib: ensure(post_template)
    Lib->>NPA: start_subsystem()
    Lib->>WorkerSub: start_subsystem()
    Lib->>Lib: catch_signals() & handle_shutdown_requests()

    Note over NPA: Краулер работает как подсистема по интервалу
    loop Каждые npa_interval_secs
        NPA->>NPA: try_fetch_data_with_retry()
        NPA->>NPA: NpaListCrawler.fetch()
        alt Новые элементы найдены
            NPA->>WorkerSub: send(items)
        else Нет новых элементов
            NPA->>NPA: log("no items")
        else Ошибка NPA
            NPA->>RSS: RssCrawler.fetch() (fallback)
            alt RSS вернул элементы
                RSS->>WorkerSub: send(items)
            else Оба упали после ретраев
                NPA->>NPA: request_shutdown()
            end
        end
    end

    Note over WorkerSub: Worker ждёт элементы и обрабатывает батчами
    WorkerSub->>Worker: new()
    loop Каждое сообщение из канала
        WorkerSub->>Worker: process_items(items)

        loop Для каждого элемента
            Worker->>Worker: require(project_id)
            Worker->>CacheManager: has_data()
            alt Данные есть
                Worker->>CacheManager: load_cached_data()
                CacheManager->>Worker: markdown
            else Нет данных
                Worker->>MarkdownFetcher: fetch_markdown(project_id)
                MarkdownFetcher->>Worker: (docx_bytes, markdown)
                Worker->>CacheManager: save_artifacts(docx, markdown)
            end

            Worker->>CacheManager: has_summary()
            alt Summary есть
                Worker->>CacheManager: load_summary()
            else Нет summary
                Worker->>Worker: throttle_by_poll_delay()
                Worker->>Summarizer: summarize_with_limit(title, markdown, url, limit?)
                Summarizer->>Worker: summary_text
                Worker->>CacheManager: save_artifacts(summary)
            end

            Worker->>ChannelMgr: get_enabled_channels()
            loop Для каждого канала
                Worker->>CacheManager: is_published_in_channel?
                alt Уже опубликовано
                    Worker->>Worker: skip
                else Нужна публикация
                    Worker->>CacheManager: has_channel_summary()
                    alt Есть
                        Worker->>CacheManager: load_channel_summary()
                    else Нет
                        Worker->>Summarizer: summarize_with_limit(... channel_limit)
                        Summarizer->>Worker: channel_summary
                        Worker->>CacheManager: save_channel_summary()
                    end

                    Worker->>CacheManager: has_channel_post()
                    alt Есть
                        Worker->>CacheManager: load_channel_post()
                    else Нет
                        Worker->>Worker: build_post(tera template)
                        Worker->>CacheManager: save_channel_post()
                    end

                    Worker->>Publishers: publish(channel, post)
                    Publishers->>Worker: ok?
                    alt success
                        Worker->>CacheManager: add_published_channels()
                    else failure
                        Worker->>Worker: continue
                    end
                end
            end
        end

        alt Достигнут лимит max_posts_per_run
            WorkerSub->>WorkerSub: break loop
            WorkerSub->>WorkerSub: request_shutdown()
        end
    end

    Note over App: Graceful Shutdown (Ctrl+C или запрос из подсистем)
    App->>NPA: on_shutdown_requested()
    App->>WorkerSub: on_shutdown_requested()
    NPA->>NPA: cancel_on_shutdown()
    WorkerSub->>WorkerSub: cancel_on_shutdown()
    App->>App: shutdown_complete()
```

## Компоненты системы


### Worker
- **Назначение**: Асинхронная обработка элементов краулинга с поэтапной проверкой кэша
- **Основные функции**:
  - Запускается в отдельной задаче с SubsystemHandle и работает независимо
  - Ожидает сообщения из канала от NPA краулера
  - Поддержка graceful shutdown через tokio::select!
  - Поэтапная проверка кэша: данные → суммаризация → статус публикации
  - Скачивание данных только при необходимости
  - Суммаризация текста через AI только при необходимости
  - Публикация только в неопубликованные каналы
  - Построение постов из шаблонов
  - Корректное сохранение состояния при завершении

### Crawlers
- **NpaListCrawler**: Читает данные с regulation.gov.ru
  - Всегда начинает с offset=0 (последние новости)
  - При отсутствии новых данных углубляется в историю
  - Сохраняет прогресс в manifest.json
  - При ошибках самостоятельно запускает RssCrawler как fallback
- **RssCrawler**: Fallback источник данных
  - Запускается NpaListCrawler при ошибках
  - Парсит RSS фид

### Publishers
- **Console**: Вывод в консоль
- **File**: Сохранение в файл
- **Telegram**: Публикация в Telegram
- **Mastodon**: Публикация в Mastodon

### CacheManager
- **Назначение**: Поэтапное кэширование артефактов для оптимизации
- **Структура**:
  - Метаданные проекта (проверка наличия данных)
  - Исходные документы (docx) - кэшируются после скачивания
  - Извлеченный markdown - кэшируется после парсинга
  - Суммаризированный текст - кэшируется после AI обработки
  - Финальные посты - кэшируются после построения
  - Статус публикации по каналам - отслеживает где уже опубликовано

## Поток данных

1. **Инициализация**: Загрузка конфигурации, инициализация сервисов
2. **Запуск асинхронных задач**: NPA краулер и Worker запускаются в отдельных задачах
3. **Независимая работа**: NPA краулер и Worker работают независимо друг от друга
4. **Периодический краулинг**: NPA краулер работает по расписанию и отправляет данные в канал
5. **Событийная обработка**: Worker ждет сообщения из канала и обрабатывает их
6. **Поэтапная обработка**: Worker проверяет кэш на каждом этапе:
   - Проверка наличия данных → скачивание при необходимости
   - Проверка наличия суммаризации → AI обработка при необходимости  
   - Проверка статуса публикации → публикация только в новые каналы
7. **Кэширование**: Все артефакты сохраняются поэтапно для оптимизации

## Graceful Shutdown

Приложение поддерживает корректное завершение работы через `tokio_graceful_shutdown`:

### Механизм работы:
- **Перехват сигналов**: `Toplevel::catch_signals()` перехватывает Ctrl+C
- **SubsystemHandle**: Каждый компонент получает handle для отслеживания shutdown
- **tokio::select!**: Компоненты используют select для ожидания shutdown или основной работы
- **Корректное завершение**: Сохранение состояния кэша и закрытие соединений

### Пример реализации:
```rust
async fn worker_subsystem(subsys: SubsystemHandle) -> Result<()> {
    tokio::select! {
        _ = subsys.on_shutdown_requested() => {
            tracing::info!("Worker shutdown requested");
            // Сохранение финального состояния
            save_final_state().await?;
        },
        _ = worker_main_loop() => {
            // Основная работа worker
        }
    };
    Ok(())
}
```

## Особенности архитектуры

- **Полная асинхронность**: NPA краулер и Worker работают в отдельных задачах независимо
- **Graceful Shutdown**: Поддержка корректного завершения по Ctrl+C через tokio_graceful_shutdown
- **Разделение ответственности**: Lib только запускает задачи, не участвует в их работе
- **Событийно-ориентированная обработка**: Worker ждет сообщения из канала, а не работает по таймеру
- **Отказоустойчивость**: NpaListCrawler самостоятельно запускает RssCrawler при ошибках
- **Простота**: Worker просто ждет данные из канала без таймаутов
- **Поэтапное кэширование**: Проверка кэша на каждом этапе (данные → суммаризация → публикация)
- **Оптимизация**: Избежание повторной обработки на любом этапе
- **Селективная публикация**: Публикация только в неопубликованные каналы
- **Модульность**: Четкое разделение ответственности
- **Конфигурируемость**: Все параметры настраиваются через config.yaml
