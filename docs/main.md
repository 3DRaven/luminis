# Luminis - Архитектура приложения

## Диаграмма последовательности основного потока

```mermaid
sequenceDiagram
    participant App as Application
    participant Lib as lib.rs
    participant Subsys as Subsystem
    participant NPA as NpaListCrawler
    participant RSS as RssCrawler
    participant Worker as Worker
    participant MarkdownFetcher as MarkdownFetcher
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
    Lib->>Lib: run_worker()

    Note over Lib: Инициализация сервисов
    Lib->>Lib: init_chat_api()
    Lib->>Lib: init_summarizer()
    Lib->>Lib: init_telegram_api()
    Lib->>Lib: init_mastodon()
    Lib->>Subsys: new()
    Lib->>Worker: new()

    Note over Subsys: Запуск асинхронных задач
    Lib->>Subsys: start_npa_crawler()
    Subsys->>Subsys: spawn(async_task)
    
    Note over Lib: Запуск Worker с поддержкой graceful shutdown
    Lib->>Worker: start_processing_with_shutdown()
    Worker->>Worker: spawn(async_task_with_shutdown_handle)
    
    Note over NPA: NPA краулер работает независимо
    loop Каждый npa_interval_secs
        NPA->>NPA: fetch()
        NPA->>NPA: load_manifest()
        NPA->>NPA: fetch_latest(offset=0)
        
        alt Новые элементы найдены
            NPA->>Worker: send_items_to_channel()
        else Новых элементов нет
            NPA->>NPA: fetch_history(last_offset)
            NPA->>NPA: update_manifest()
            NPA->>Worker: send_items_to_channel()
        else Ошибка NPA
            NPA->>RSS: fetch() (fallback)
            RSS->>Worker: send_items_to_channel()
        end
    end

    Note over Worker: Worker работает независимо
    loop Каждое сообщение из канала
        Worker->>Worker: receive_items_from_channel()
        
        loop Для каждого элемента
            Worker->>Worker: check_cache_metadata()
            
            alt Данные уже скачаны
                Worker->>CacheManager: load_cached_data()
                CacheManager->>Worker: cached_data
            else Данные не скачаны
                Worker->>MarkdownFetcher: fetch_markdown()
                MarkdownFetcher->>MarkdownFetcher: get_file_id()
                MarkdownFetcher->>MarkdownFetcher: download_file()
                MarkdownFetcher->>Worker: (bytes, text)
                Worker->>CacheManager: save_cache_artifacts()
            end
            
            Worker->>Worker: check_cache_summary()
            
            alt Суммаризация уже готова
                Worker->>CacheManager: load_cached_summary()
                CacheManager->>Worker: cached_summary
            else Суммаризация нужна
                Worker->>Summarizer: summarize_with_limit()
                Summarizer->>Summarizer: call_chat_api()
                Summarizer->>Worker: summary_text
                Worker->>CacheManager: save_cache_artifacts()
            end
            
            Worker->>Worker: check_published_channels()
            Worker->>CacheManager: load_cache_metadata()
            CacheManager->>Worker: published_channels
            
            alt Нужна публикация
                Worker->>Worker: build_post()
                Worker->>Publishers: publish()
                Publishers->>Worker: published_names
                Worker->>CacheManager: add_published_channels()
            else Все каналы уже опубликованы
                Worker->>Worker: skip_publishing()
            end
        end
    end
    
    Note over App: Graceful Shutdown (Ctrl+C)
    App->>App: catch_signals()
    App->>NPA: on_shutdown_requested()
    App->>Worker: on_shutdown_requested()
    
    alt Shutdown запрошен
        NPA->>NPA: graceful_shutdown()
        Worker->>Worker: graceful_shutdown()
        App->>App: shutdown_complete()
    end
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
