//! Сетевой стек операционной системы Zarya
//! 
//! Базовая реализация TCP/IP стека.

use crate::sync::Spinlock;

/// Инициализация сетевого стека
pub fn init() {
    kernel_log!("[NET] Network stack initialized\n");
}

/// IP адрес
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpAddr {
    pub octets: [u8; 4],
}

impl IpAddr {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddr { octets: [a, b, c, d] }
    }
    
    pub const LOCALHOST: Self = IpAddr::new(127, 0, 0, 1);
    pub const ANY: Self = IpAddr::new(0, 0, 0, 0);
}

/// MAC адрес
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddr {
    pub octets: [u8; 6],
}

impl MacAddr {
    pub const fn new(octets: [u8; 6]) -> Self {
        MacAddr { octets }
    }
    
    pub const fn zero() -> Self {
        MacAddr { octets: [0; 6] }
    }
}

/// Сокет
pub struct Socket {
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: Option<IpAddr>,
    pub remote_port: Option<u16>,
    pub protocol: Protocol,
    pub state: SocketState,
}

/// Типы протоколов
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
}

/// Состояния сокета
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Сетевой интерфейс
pub struct NetworkInterface {
    pub name: String,
    pub mac: MacAddr,
    pub ip: IpAddr,
    pub netmask: IpAddr,
    pub gateway: Option<IpAddr>,
    pub mtu: u16,
    pub up: bool,
}

impl NetworkInterface {
    pub fn new(name: &str) -> Self {
        NetworkInterface {
            name: name.to_string(),
            mac: MacAddr::zero(),
            ip: IpAddr::ANY,
            netmask: IpAddr::new(255, 255, 255, 0),
            gateway: None,
            mtu: 1500,
            up: false,
        }
    }
}

/// Глобальный список интерфейсов
static INTERFACES: Spinlock<Vec<NetworkInterface>> = Spinlock::new(Vec::new());

/// Добавление сетевого интерфейса
pub fn add_interface(iface: NetworkInterface) {
    INTERFACES.lock().push(iface);
}

/// Отправка пакета
pub fn send_packet(data: &[u8], dest: &IpAddr) -> Result<usize, &'static str> {
    kernel_log!("[NET] Sending packet to {:?}\n", dest.octets);
    // В реальной системе отправка через драйвер
    Ok(data.len())
}

/// Получение пакета
pub fn recv_packet(buf: &mut [u8]) -> Result<usize, &'static str> {
    // В реальной системе получение из драйвера
    Ok(0)
}

/// Создание сокета
pub fn socket(protocol: Protocol) -> Result<Socket, &'static str> {
    let socket = Socket {
        local_addr: IpAddr::ANY,
        local_port: 0,
        remote_addr: None,
        remote_port: None,
        protocol,
        state: SocketState::Closed,
    };
    
    Ok(socket)
}

/// Привязка сокета к адресу
pub fn bind(socket: &mut Socket, addr: IpAddr, port: u16) -> Result<(), &'static str> {
    socket.local_addr = addr;
    socket.local_port = port;
    Ok(())
}

/// Подключение сокета
pub fn connect(socket: &mut Socket, addr: IpAddr, port: u16) -> Result<(), &'static str> {
    socket.remote_addr = Some(addr);
    socket.remote_port = Some(port);
    socket.state = SocketState::SynSent;
    Ok(())
}

/// Отправка данных через сокет
pub fn send(socket: &Socket, data: &[u8]) -> Result<usize, &'static str> {
    if socket.state != SocketState::Established {
        return Err("Socket not connected");
    }
    
    kernel_log!("[NET] Sending {} bytes\n", data.len());
    Ok(data.len())
}

/// Получение данных через сокет
pub fn recv(socket: &Socket, buf: &mut [u8]) -> Result<usize, &'static str> {
    if socket.state != SocketState::Established {
        return Err("Socket not connected");
    }
    
    Ok(0)
}

/// Закрытие сокета
pub fn close_socket(socket: &mut Socket) {
    socket.state = SocketState::Closed;
}
