//! # Сетевой стек операционной системы Zarya
//!
//! Базовая реализация TCP/IP стека.
//! Поддерживает Ethernet, IPv4, UDP, TCP.

#![no_std]

use spin::Mutex;
use alloc::vec::Vec;

/// Глобальное состояние сети
static NETWORK_INITIALIZED: Mutex<bool> = Mutex::new(false);

/// MAC адрес (6 байт)
pub type MacAddress = [u8; 6];

/// IPv4 адрес (4 байта)
pub type IpAddress = [u8; 4];

/// Сетевой интерфейс
pub struct NetworkInterface {
    pub name: [u8; 16],
    pub mac: MacAddress,
    pub ip: IpAddress,
    pub netmask: IpAddress,
    pub gateway: IpAddress,
    pub mtu: u16,
    pub flags: InterfaceFlags,
}

/// Флаги интерфейса
pub struct InterfaceFlags {
    pub up: bool,
    pub broadcast: bool,
    pub multicast: bool,
    pub running: bool,
}

/// Ethernet кадр
#[repr(C, packed)]
pub struct EthernetFrame {
    pub dest_mac: MacAddress,
    pub src_mac: MacAddress,
    pub ethertype: u16,
    // Данные...
}

/// IPv4 пакет
#[repr(C, packed)]
pub struct Ipv4Packet {
    pub version_ihl: u8,
    pub dscp_ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags_fragment: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: IpAddress,
    pub dest_ip: IpAddress,
    // Опции и данные...
}

/// TCP заголовок
#[repr(C, packed)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset_flags: u16,
    pub window: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

/// UDP заголовок
#[repr(C, packed)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub length: u16,
    pub checksum: u16,
}

/// Типы протоколов
pub mod protocols {
    pub const ICMP: u8 = 1;
    pub const TCP: u8 = 6;
    pub const UDP: u8 = 17;
}

/// Сокет
pub struct Socket {
    pub id: usize,
    pub domain: SocketDomain,
    pub sock_type: SocketType,
    pub protocol: u8,
    pub local_addr: Option<SockAddr>,
    pub remote_addr: Option<SockAddr>,
    pub state: SocketState,
    pub recv_buffer: Vec<u8>,
    pub send_buffer: Vec<u8>,
}

/// Домен сокета
#[derive(Debug, Clone, Copy)]
pub enum SocketDomain {
    Inet,      // IPv4
    Inet6,     // IPv6
    Unix,      // Локальный
}

/// Тип сокета
#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    Stream,    // TCP
    Datagram,  // UDP
    Raw,       // Сырой
}

/// Адрес сокета
pub struct SockAddr {
    pub ip: IpAddress,
    pub port: u16,
}

/// Состояние TCP сокета
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SocketState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// Сетевой стек
pub struct NetworkStack {
    interfaces: Vec<NetworkInterface>,
    sockets: Vec<Socket>,
    next_socket_id: usize,
}

impl NetworkStack {
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            sockets: Vec::new(),
            next_socket_id: 1,
        }
    }
    
    /// Добавление сетевого интерфейса
    pub fn add_interface(&mut self, iface: NetworkInterface) {
        self.interfaces.push(iface);
    }
    
    /// Создание сокета
    pub fn socket(&mut self, domain: SocketDomain, sock_type: SocketType, protocol: u8) -> Result<usize, &'static str> {
        let id = self.next_socket_id;
        self.next_socket_id += 1;
        
        let socket = Socket {
            id,
            domain,
            sock_type,
            protocol,
            local_addr: None,
            remote_addr: None,
            state: SocketState::Closed,
            recv_buffer: Vec::new(),
            send_buffer: Vec::new(),
        };
        
        self.sockets.push(socket);
        Ok(id)
    }
    
    /// Привязка сокета к адресу
    pub fn bind(&mut self, socket_id: usize, addr: SockAddr) -> Result<(), &'static str> {
        if let Some(socket) = self.sockets.iter_mut().find(|s| s.id == socket_id) {
            socket.local_addr = Some(addr);
            Ok(())
        } else {
            Err("Socket not found")
        }
    }
    
    /// Прослушивание сокета (TCP)
    pub fn listen(&mut self, socket_id: usize, backlog: usize) -> Result<(), &'static str> {
        if let Some(socket) = self.sockets.iter_mut().find(|s| s.id == socket_id) {
            if socket.sock_type != SocketType::Stream {
                return Err("Not a stream socket");
            }
            socket.state = SocketState::Listen;
            Ok(())
        } else {
            Err("Socket not found")
        }
    }
    
    /// Принятие соединения (TCP)
    pub fn accept(&mut self, socket_id: usize) -> Result<(usize, SockAddr), &'static str> {
        // Упрощенная реализация
        Err("Not implemented")
    }
    
    /// Подключение к удаленному адресу
    pub fn connect(&mut self, socket_id: usize, addr: SockAddr) -> Result<(), &'static str> {
        if let Some(socket) = self.sockets.iter_mut().find(|s| s.id == socket_id) {
            socket.remote_addr = Some(addr);
            // В полной версии здесь была бы установка TCP соединения
            socket.state = SocketState::Established;
            Ok(())
        } else {
            Err("Socket not found")
        }
    }
    
    /// Отправка данных
    pub fn send(&mut self, socket_id: usize, data: &[u8]) -> Result<usize, &'static str> {
        if let Some(socket) = self.sockets.iter_mut().find(|s| s.id == socket_id) {
            if socket.state != SocketState::Established {
                return Err("Socket not connected");
            }
            
            // Копирование в буфер отправки
            for byte in data {
                socket.send_buffer.push(*byte);
            }
            
            // В полной версии здесь была бы отправка пакета
            Ok(data.len())
        } else {
            Err("Socket not found")
        }
    }
    
    /// Получение данных
    pub fn recv(&mut self, socket_id: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
        if let Some(socket) = self.sockets.iter_mut().find(|s| s.id == socket_id) {
            if socket.recv_buffer.is_empty() {
                return Ok(0);
            }
            
            let len = core::cmp::min(buf.len(), socket.recv_buffer.len());
            buf[..len].copy_from_slice(&socket.recv_buffer[..len]);
            
            // Удаление прочитанных данных
            socket.recv_buffer.drain(..len);
            
            Ok(len)
        } else {
            Err("Socket not found")
        }
    }
    
    /// Закрытие сокета
    pub fn close(&mut self, socket_id: usize) -> Result<(), &'static str> {
        if let Some(pos) = self.sockets.iter().position(|s| s.id == socket_id) {
            self.sockets.remove(pos);
            Ok(())
        } else {
            Err("Socket not found")
        }
    }
}

/// Инициализация сети
pub fn init() -> NetworkStack {
    let mut stack = NetworkStack::new();
    
    // Создание loopback интерфейса
    let loopback = NetworkInterface {
        name: *b"lo\0\0\0\0\0\0\0\0\0\0\0\0",
        mac: [0, 0, 0, 0, 0, 0],
        ip: [127, 0, 0, 1],
        netmask: [255, 0, 0, 0],
        gateway: [0, 0, 0, 0],
        mtu: 65535,
        flags: InterfaceFlags {
            up: true,
            broadcast: false,
            multicast: false,
            running: true,
        },
    };
    
    stack.add_interface(loopback);
    
    *NETWORK_INITIALIZED.lock() = true;
    
    println!("[NETWORK] Сетевой стек инициализирован");
    
    stack
}

/// Проверка инициализации сети
pub fn is_initialized() -> bool {
    *NETWORK_INITIALIZED.lock()
}

/// Отправка Ethernet кадра
pub fn send_ethernet_frame(iface: &NetworkInterface, frame: &EthernetFrame) {
    // В полной версии здесь была бы отправка через драйвер сетевой карты
    println!("[NETWORK] Отправка Ethernet кадра через {}", 
             core::str::from_utf8(&iface.name).unwrap_or("unknown"));
}

/// Обработка входящего пакета
pub fn handle_packet(data: &[u8]) {
    if data.len() < 14 {
        return;
    }
    
    // Parse Ethernet header
    let ethertype = u16::from_be_bytes([data[12], data[13]]);
    
    match ethertype {
        0x0800 => {
            // IPv4
            println!("[NETWORK] Получен IPv4 пакет");
            handle_ipv4(&data[14..]);
        }
        0x0806 => {
            // ARP
            println!("[NETWORK] Получен ARP пакет");
        }
        _ => {
            println!("[NETWORK] Неизвестный тип пакета: {:#x}", ethertype);
        }
    }
}

/// Обработка IPv4 пакета
fn handle_ipv4(data: &[u8]) {
    if data.is_empty() {
        return;
    }
    
    let protocol = data[9];
    
    match protocol {
        protocols::ICMP => println!("[NETWORK] ICMP пакет"),
        protocols::TCP => println!("[NETWORK] TCP пакет"),
        protocols::UDP => println!("[NETWORK] UDP пакет"),
        _ => println!("[NETWORK] Неизвестный протокол: {}", protocol),
    }
}
