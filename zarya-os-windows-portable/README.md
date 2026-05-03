# Операционная система «Zarya» (Рассвет)

![Zarya OS](docs/logo.png)

**Версия:** 1.0.0  
**Лицензия:** MIT  
**Архитектура:** x86_64  
**Статус:** Alpha

## Описание

Zarya — это современная операционная система, сочетающая в себе:
- 🍎 **Плавность и эстетику macOS** (анимации, прозрачность, единый стиль)
- 🪟 **Функциональность Windows** (рабочий стол, панель задач, оконный менеджер)
- 🤖 **Отзывчивость Android** (жесты, сенсорное управление)
- 🐧 **Свободу Linux** (открытая архитектура, модульность, терминал)

## Быстрый старт (Windows)

### Требования
- Windows 10/11 (x64)
- 4 ГБ ОЗУ минимум (рекомендуется 8 ГБ)
- 2 ядра CPU
- 500 МБ свободного места на диске

### Установка и запуск

1. **Распакуйте архив** в любую папку (желательно без кириллицы в пути)
2. **Запустите `run.bat`** двойным кликом
3. Система загрузится в окне эмулятора

```
📁 Zarya-OS-Portable/
├── run.bat              ← Запустите этот файл!
├── qemu/                ← Эмулятор (встроен)
├── zarya.iso            ← Образ системы
└── docs/                ← Документация
```

### Управление

| Действие | Клавиша |
|----------|---------|
| Выход из полноэкранного режима | `Ctrl+Alt+G` |
| Освобождение курсора мыши | `Ctrl+Alt` |
| Перезагрузка системы | `Ctrl+Alt+Del` (внутри ОС) |
| Закрытие окна эмулятора | `Alt+F4` |

## Возможности системы

### ✅ Реализовано в версии 1.0

#### Ядро
- ✅ Гибридное ядро на Rust
- ✅ Вытесняющая многозадачность
- ✅ Виртуальная память (paging)
- ✅ IPC (межпроцессное взаимодействие)
- ✅ Драйверы: клавиатура, мышь, framebuffer, serial

#### Графический интерфейс
- ✅ Дисплейный сервер ZDS
- ✅ Оконный менеджер с анимациями
- ✅ Рабочий стол с иконками
- ✅ Панель задач (Dock)
- ✅ Меню приложений (аналог Пуск)
- ✅ Экран входа (Login Screen)

#### Приложения
- ✅ **Zarya Files** — файловый менеджер
- ✅ **Zarya Text** — текстовый редактор
- ✅ **Zarya Terminal** — эмулятор терминала
- ✅ **Zarya Settings** — панель настроек

#### Файловая система
- ✅ Виртуальная ФС (VFS)
- ✅ Поддержка ZFS (базовая)
- ✅ Совместимость с FAT32/ext4

### 🚧 В разработке

- Аппаратное ускорение GPU
- Сетевой стек TCP/IP (полная реализация)
- Wi-Fi и Bluetooth драйверы
- Магазин приложений
- Облачная синхронизация

## Структура проекта

```
zarya-os/
├── boot/                 # Загрузчик (UEFI/Multiboot)
├── kernel/               # Ядро системы
│   ├── src/
│   │   ├── main.rs       # Точка входа
│   │   ├── memory/       # Управление памятью
│   │   ├── task/         # Планировщик задач
│   │   ├── ipc/          # Межпроцессное взаимодействие
│   │   ├── drivers/      # Драйверы устройств
│   │   ├── fs/           # Файловая система
│   │   ├── syscall/      # Системные вызовы
│   │   ├── net/          # Сеть
│   │   └── sync/         # Синхронизация
│   └── Cargo.toml
├── servers/              # Системные серверы
│   ├── zds/              # Display Server
│   ├── input/            # Сервер ввода
│   ├── fs/               # Файловый сервер
│   └── network/          # Сетевой сервер
├── apps/                 # Пользовательские приложения
│   ├── zarya-desktop/    # Рабочий стол
│   ├── zarya-files/      # Файловый менеджер
│   ├── zarya-text/       # Текстовый редактор
│   ├── zarya-login/      # Экран входа
│   ├── zarya-terminal/   # Терминал
│   └── zarya-settings/   # Настройки
├── docs/                 # Документация
├── build.sh              # Скрипт сборки (Linux)
├── Makefile              # Сборка и запуск
└── README.md
```

## Архитектура

### Уровень ядра (Kernel Space)
```
┌─────────────────────────────────────────────────────────┐
│                    Приложения (User Space)               │
├─────────────────────────────────────────────────────────┤
│  ZDS  │  Input  │   FS   │ Network │  Settings  │ ...   │
├─────────────────────────────────────────────────────────┤
│              Системные вызовы (Syscalls)                 │
├─────────────────────────────────────────────────────────┤
│  Memory  │  Tasks  │  IPC  │ Drivers │  FS/VFS   │ Net  │
│                      ЯДРО (Kernel)                       │
└─────────────────────────────────────────────────────────┘
```

### Компоненты ядра

| Компонент | Описание |
|-----------|----------|
| **Memory Manager** | Физический аллокатор (buddy), виртуальная память, таблицы страниц |
| **Task Scheduler** | Процессы, потоки, приоритеты, round-robin + real-time |
| **IPC** | Порты, сообщения, разделяемая память (L4-стиль) |
| **Drivers** | PS/2, USB HID, AHCI/NVMe, Framebuffer, Serial |
| **VFS** | Абстрактный слой ФС,.mount points, namespace |
| **Syscall** | Диспетчеризация, POSIX + Zarya extensions |

## Системные вызовы

Zarya поддерживает POSIX-совместимые вызовы и собственные расширения:

```rust
// Стандартные POSIX
sys_open(path, flags) → fd
sys_read(fd, buf, count) → bytes_read
sys_write(fd, buf, count) → bytes_written
sys_close(fd) → result
sys_mmap(addr, len, prot, flags, fd, offset) → ptr
sys_fork() → pid
sys_exec(path, argv, envp) → result
sys_exit(code)

// Расширения Zarya
sys_zport_create(name) → port_id
sys_zport_send(port_id, msg) → result
sys_zport_recv(port_id, timeout) → msg
sys_zwindow_create(title, width, height) → window_id
sys_zwindow_draw(window_id, buffer) → result
```

## Сборка из исходников

### Требования для сборки
- Rust nightly (`rustup default nightly`)
- NASM assembler
- GRUB или systemd-boot
- QEMU (для тестирования)
- xorriso (для создания ISO)

### Команды сборки

```bash
# Сборка ядра
cd kernel
cargo build --release --target x86_64-zarya.json

# Создание образа
./build.sh

# Запуск в QEMU
make run
```

## Отладка

### Логирование через serial

```bash
qemu-system-x86_64 -kernel zarya.bin -serial stdio
```

### GDB отладка

```bash
# В одном терминале
qemu-system-x86_64 -kernel zarya.bin -s -S

# В другом терминале
gdb kernel/target/x86_64-zarya/release/zarya
(gdb) target remote :1234
(gdb) continue
```

## Лицензия

MIT License — см. файл [LICENSE](LICENSE)

## Авторы

- Ядро и системные компоненты: Rust Team
- GUI и приложения: Zarya Team
- Дизайн: Inspired by macOS, Windows, Android, Linux

## Контакты

- Website: https://zarya-os.dev (placeholder)
- GitHub: https://github.com/zarya-os/zarya
- Discord: https://discord.gg/zarya-os (placeholder)

---

**«Zarya» — Рассвет новой эры операционных систем** 🌅
