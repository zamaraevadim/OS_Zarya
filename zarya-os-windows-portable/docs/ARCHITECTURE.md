# Архитектура операционной системы «Zarya»

## Обзор

Zarya — это гибридная операционная система с микроядерной архитектурой, где большинство сервисов выполняются в пользовательском пространстве для повышения надёжности и безопасности.

## Уровни системы

```
┌─────────────────────────────────────────────────────────────────────┐
│                         ПРИЛОЖЕНИЯ (Applications)                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │  Files   │ │   Text   │ │ Terminal │ │ Settings │ │  Login   │  │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│                      СИСТЕМНЫЕ СЕРВЕРЫ (Servers)                     │
│  ┌────────────────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐ │
│  │  ZDS (Display)     │ │  Input   │ │    FS    │ │   Network    │ │
│  │  - Композитинг     │ │  Сервер  │ │  Сервер  │ │    Сервер    │ │
│  │  - Рендеринг окон  │ │  ввода   │ │  файлов  │ │  TCP/IP стек │ │
│  └────────────────────┘ └──────────┘ └──────────┘ └──────────────┘ │
├─────────────────────────────────────────────────────────────────────┤
│                   БИБЛИОТЕКИ ПОЛЬЗОВАТЕЛЬСКОГО ПРОСТРАНСТВА          │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  libzarya (C API + Rust bindings)                             │   │
│  │  - Системные вызовы                                           │   │
│  │  - IPC клиенты                                                │   │
│  │  - Графический тулкит ZUI                                     │   │
│  └──────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│                       ГРАНИЦА ЯДРО/ПОЛЬЗОВАТЕЛЬ (Syscall)           │
├─────────────────────────────────────────────────────────────────────┤
│                            ЯДРО (Kernel)                             │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌─────────────────┐  │
│  │   Memory   │ │    Task    │ │    IPC     │ │    Drivers      │  │
│  │  Manager   │ │ Scheduler  │ │  Subsystem │ │   Framework     │  │
│  │            │ │            │ │            │ │                 │  │
│  │ - Buddy    │ │ - Процессы │ │ - Порты    │ │ - Модули        │  │
│  │ - Paging   │ │ - Потоки   │ │ - Сообщения│ │ - Hotplug       │  │
│  │ - VMA      │ │ - Priority │ │ - Shared   │ │ - PM            │  │
│  │            │ │            │ │   Memory   │ │                 │  │
│  └────────────┘ └────────────┘ └────────────┘ └─────────────────┘  │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌─────────────────┐  │
│  │     VFS    │ │   Syscall  │ │    Net     │ │     Sync        │  │
│  │            │ │ Dispatcher │ │   Stack    │ │   Primitives    │  │
│  │ - Mount    │ │            │ │            │ │                 │  │
│  │ - Namespace│ │ - Таблица  │ │ - IPv4/IPv6│ │ - Spinlock      │  │
│  │ - Cache    │ │ - Handler  │ │ - Sockets  │ │ - Mutex         │  │
│  │            │ │            │ │            │ │ - Semaphore     │  │
│  └────────────┘ └────────────┘ └────────────┘ └─────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│                     АППАРАТНЫЙ УРОВЕНЬ (Hardware)                    │
│  CPU (x86_64) │ RAM │ MMU │ APIC │ PCI(e) │ SATA/NVMe │ GPU/FB    │
└─────────────────────────────────────────────────────────────────────┘
```

## Компоненты ядра

### 1. Менеджер памяти (Memory Manager)

**Ответственность:**
- Управление физической памятью (buddy allocator)
- Виртуальная память с таблицами страниц (4-уровневые для x86_64)
- Выделение областей памяти (VMA — Virtual Memory Areas)
- Copy-on-write для fork()
- Swapping (в перспективе)

**Структуры данных:**
```rust
struct PhysicalAllocator {
    zones: [Zone; MAX_ZONES],
    free_lists: [LinkedList; MAX_ORDER],
}

struct VirtualMemorySpace {
    page_tables: PageTables,
    vmas: IntervalTree<VMA>,
    mmap_base: VirtAddr,
}
```

### 2. Планировщик задач (Task Scheduler)

**Классы планирования:**
- **SCHED_NORMAL**: Обычные процессы (CFS-like)
- **SCHED_BATCH**: Пакетные задачи (фоновые)
- **SCHED_FIFO**: Real-time FIFO
- **SCHED_RR**: Real-time Round-Robin

**Структура процесса:**
```rust
struct Process {
    pid: Pid,
    ppid: Pid,
    state: ProcessState,
    memory_space: Arc<AddressSpace>,
    threads: Vec<Arc<Thread>>,
    files: FileTable,
    signals: SignalHandlers,
}

struct Thread {
    tid: Tid,
    state: ThreadState,
    stack: KernelStack,
    context: CpuContext,
    priority: Priority,
}
```

### 3. Межпроцессное взаимодействие (IPC)

**Модель L4-стиля:**
- Синхронная передача сообщений
- Endpoints (порты) для коммуникации
- Разделяемая память для больших данных
- Capability-based security

**API:**
```rust
// Создание порта
port_id = sys_zport_create(name: &str) -> Result<PortId>

// Отправка сообщения
sys_zport_send(port_id: PortId, msg: &Message) -> Result<usize>

// Получение сообщения
msg = sys_zport_recv(port_id: PortId, timeout: Option<Duration>) -> Result<Message>

// Отображение разделяемой памяти
ptr = sys_shm_map(shm_id: ShmId, offset: usize, size: usize) -> Result<*mut u8>
```

### 4. Драйверы (Drivers)

**Модель драйверов:**
- Загружаемые модули (ELF формат)
- Выполнение в user-space с минимальными привилегиями
- Доступ к оборудованию через kernel API

**Поддерживаемые устройства:**
| Устройство | Тип | Статус |
|------------|-----|--------|
| Клавиатура PS/2 | Input | ✅ Готово |
| Мышь PS/2 | Input | ✅ Готово |
| USB HID | Input | 🚧 В разработке |
| Framebuffer (GOP/VBE) | Display | ✅ Готово |
| AHCI/SATA | Storage | ✅ Готово |
| NVMe | Storage | 🚧 В разработке |
| Intel E1000 | Network | 🚧 В разработке |
| Serial (16550) | Debug | ✅ Готово |

### 5. Виртуальная файловая система (VFS)

**Иерархия:**
```
/
├── bin/          # Системные утилиты
├── dev/          # Устройства (device nodes)
├── etc/          # Конфигурация
├── home/         # Домашние директории пользователей
│   └── user/
│       ├── Documents/
│       ├── Downloads/
│       ├── Pictures/
│       └── ...
├── mnt/          # Точки монтирования
├── proc/         # Информация о процессах (псевдо-ФС)
├── sys/          # Информация о системе (псевдо-ФС)
├── tmp/          # Временные файлы
└── var/          # Переменные данные
    └── log/      # Логи системы
```

**Операции VFS:**
```rust
trait VfsOperations {
    fn open(&self, path: &Path, flags: OpenFlags) -> Result<FileDescriptor>;
    fn read(&self, fd: FileDescriptor, buf: &mut [u8]) -> Result<usize>;
    fn write(&self, fd: FileDescriptor, buf: &[u8]) -> Result<usize>;
    fn close(&self, fd: FileDescriptor) -> Result<()>;
    fn mkdir(&self, path: &Path, mode: Mode) -> Result<()>;
    fn unlink(&self, path: &Path) -> Result<()>;
    fn mount(&self, fs_type: &str, device: &str, mountpoint: &Path) -> Result<()>;
}
```

### 6. Системные вызовы (Syscalls)

**Таблица системных вызовов:**

| Номер | Имя | Описание |
|-------|-----|----------|
| 0 | `sys_read` | Чтение из файла/descriptor |
| 1 | `sys_write` | Запись в файл/descriptor |
| 2 | `sys_open` | Открытие файла |
| 3 | `sys_close` | Закрытие файла |
| 4 | `sys_stat` | Получение информации о файле |
| 5 | `sys_mmap` | Отображение памяти |
| 6 | `sys_munmap` | Отмена отображения памяти |
| 7 | `sys_fork` | Создание дочернего процесса |
| 8 | `sys_exec` | Выполнение программы |
| 9 | `sys_exit` | Завершение процесса |
| 10 | `sys_wait` | Ожидание дочернего процесса |
| ... | ... | ... |
| 100 | `sys_zport_create` | Создание IPC порта (Zarya) |
| 101 | `sys_zport_send` | Отправка IPC сообщения (Zarya) |
| 102 | `sys_zport_recv` | Получение IPC сообщения (Zarya) |
| 103 | `sys_zwindow_create` | Создание окна (Zarya) |
| 104 | `sys_zwindow_draw` | Отрисовка окна (Zarya) |

**Механизм вызова (x86_64):**
```asm
; Вызов через syscall инструкцию
mov rax, <syscall_number>  ; Номер вызова
mov rdi, <arg1>            ; Аргумент 1
mov rsi, <arg2>            ; Аргумент 2
mov rdx, <arg3>            ; Аргумент 3
mov r10, <arg4>            ; Аргумент 4
mov r8, <arg5>             ; Аргумент 5
mov r9, <arg6>             ; Аргумент 6
syscall                    ; Переход в kernel mode
; Возврат в rax
```

## Системные серверы

### ZDS (Zarya Display Server)

**Функции:**
- Приём соединений от клиентов (приложений)
- Композитинг окон (отрисовка в общий буфер)
- Обработка событий ввода (мышь, клавиатура, тач)
- Управление виртуальными рабочими столами
- Анимации и эффекты

**Протокол общения:**
```
Client → ZDS:
  - CreateWindow(title, width, height, flags)
  - DrawWindow(window_id, buffer, damage_rects)
  - DestroyWindow(window_id)
  - SetWindowTitle(window_id, title)
  - MoveWindow(window_id, x, y)
  - ResizeWindow(window_id, width, height)

ZDS → Client:
  - MouseEvent(window_id, type, x, y, buttons)
  - KeyEvent(window_id, type, keycode, modifiers)
  - CloseRequest(window_id)
  - ResizeEvent(window_id, new_width, new_height)
```

### Input Server

**Обязанности:**
- Чтение событий от драйверов ввода
- Преобразование в унифицированный формат
- Мультиплексирование между клиентами
- Поддержка жестов (touchscreen)

### Filesystem Server

**Обязанности:**
- Монтирование устройств хранения
- Обслуживание запросов VFS
- Кэширование данных
- Поддержка snapshots (для ZFS)

## Безопасность

### Модель разрешений

Каждое приложение работает в изолированной песочнице:

```rust
struct Permissions {
    filesystem: FsPermissions,
    network: NetworkPermissions,
    devices: DevicePermissions,
    ipc: IpcPermissions,
}

struct FsPermissions {
    read_paths: Vec<PathBuf>,
    write_paths: Vec<PathBuf>,
    allow_network: bool,
    allow_devices: Vec<DeviceId>,
}
```

### Capability-based security

Доступ к ресурсам через capabilities:
```rust
struct Capability {
    resource: ResourceId,
    rights: Rights,
    expiry: Option<Instant>,
}
```

## Производительность

### Оптимизации

1. **Lock-free структуры данных** где возможно
2. **RCU (Read-Copy-Update)** для частых чтений
3. **Per-CPU данные** для избежания contention
4. **Batch allocation** в аллокаторе
5. **Zero-copy IPC** для больших сообщений

### Измерения (целевые)

| Метрика | Цель | Достигнуто |
|---------|------|------------|
| Время загрузки | < 5 сек | ~8 сек |
| Переключение контекста | < 1 μs | ~2 μs |
| IPC latency | < 10 μs | ~15 μs |
| Page fault handling | < 1 μs | ~3 μs |

## Расширяемость

### Модули ядра

Модули загружаются как ELF объекты:
```rust
#[no_mangle]
pub extern "C" fn module_init() -> Result<()> {
    // Инициализация модуля
    Ok(())
}

#[no_mangle]
pub extern "C" fn module_fini() {
    // Очистка модуля
}
```

### Пользовательские драйверы

Драйверы работают в user-space:
```rust
fn main() {
    let device = open_device("/dev/pci/00:1f.2");
    
    loop {
        let event = device.read_event();
        process_event(event);
    }
}
```

## Будущие улучшения

### Версия 1.1 (План)
- [ ] Полная поддержка сетевого стека
- [ ] Драйверы Wi-Fi
- [ ] Аппаратное ускорение GPU (Mesa)
- [ ] Контейнеризация приложений

### Версия 2.0 (План)
- [ ] Поддержка ARM64
- [ ] Secure Boot
- [ ] Шифрование диска (LUKS-совместимое)
- [ ] Магазин приложений

---

*Документ обновлён: 2024*
*Автор: Zarya OS Team*
