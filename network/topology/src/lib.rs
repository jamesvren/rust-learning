use libc::{uname, utsname};
use std::fs;
use std::io;
use std::fmt;
use std::mem;
use std::os::fd::OwnedFd;
use std::os::fd::FromRawFd;
use std::os::fd::AsRawFd;
use std::str::FromStr;
use std::collections::HashMap;
use log::{debug, error};
use tokio::io::unix::AsyncFd;

pub mod packet;

pub fn hostname() -> String {
    let mut name = utsname {
        sysname: ['\0' as i8; 65],
        nodename: ['\0' as i8; 65],
        release: ['\0' as i8; 65],
        version: ['\0' as i8; 65],
        machine: ['\0' as i8; 65],
        domainname: ['\0' as i8; 65],
    };
    let pname: *mut utsname = &mut name;

    unsafe {
        if uname(pname) == 0 {
            String::from_utf8_lossy(std::mem::transmute(&name.nodename[..])).trim_end_matches('\0').to_string()
        } else {
            String::from("")
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum OperState {
    Up,
    Down,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError;

impl FromStr for OperState {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "up" => Ok(OperState::Up),
            "down" => Ok(OperState::Down),
            _ => Err(ParseError),
        }
    }
}

impl Default for OperState {
    fn default() -> Self {
        OperState::Up
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub struct MacAddress(u8, u8, u8, u8, u8, u8);

impl FromStr for MacAddress {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: Vec<_> = s.trim().split(":").collect();
        if v.len() == 6 {
            let v: Vec<_> = v.into_iter().map(|i| u8::from_str_radix(i, 16).unwrap()).collect();
            Ok(MacAddress(v[0], v[1], v[2], v[3], v[4], v[5]))
        } else {
            Err(ParseError)
        }
    }
}

impl From<[u8; 6]> for MacAddress {
    fn from(v: [u8; 6]) -> Self {
        MacAddress (v[0], v[1], v[2], v[3], v[4], v[5])
    }
}

impl From<&MacAddress> for [u8; 6] {
    fn from(mac: &MacAddress) -> Self {
        [mac.0, mac.1, mac.2, mac.3, mac.4, mac.5]
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
               self.0, self.1, self.2, self.3, self.4, self.5)
    }
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub struct Interface {
    pub index: u32,
    pub name: String,
    pub mac: MacAddress,
    pub state: OperState,
}

#[derive(PartialEq, Eq, Hash)]
pub struct Node {
    pub host: String,
    pub nic: Interface,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.host, self.nic.name, self.nic.mac)
    }
}

pub struct Topo {
    pub connection: HashMap<(Node, Node), Vec<u16>>,
}

impl fmt::Display for Topo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for ((me, peer), vlans) in &self.connection {
            let _ = write!(f, "{} <-> {} VLAN: {}", me, peer, show_vlan(&vlans));
        }
        Ok(())
    }
}

fn show_vlan(vlans: &Vec<u16>) -> String {
    if vlans.is_empty() {
        return String::new();
    }
    let mut start = vlans.get(0);
    let mut vlan_str = String::new();
    let mut range = false;
    let len = vlans.len();

    for i in 0..len {
        let end = vlans.get(i);
        let next = vlans.get(i + 1);
        match next {
            Some(&v) => {
                if v != end.unwrap() + 1 {
                    range = true
                }
            },
            None => range = true,
        }
        if range {
            if !vlan_str.is_empty() {
                vlan_str += ",";
            }
            if start == end {
                vlan_str += &format!("{}", start.unwrap());
            } else {
                vlan_str += &format!("{}-{}", start.unwrap(), end.unwrap());
            }
            range = false;
            start = next;
        }
    }
    vlan_str
}

pub fn get_physical_nics() -> Vec<Interface> {
    const NIC_PATH: &str = "/sys/class/net/";
    let mut nics: Vec<_> = vec![];

    if let Ok(dir) = fs::read_dir(NIC_PATH) {
        for entry in dir {
            let entry = entry.unwrap();
            if let Ok(path) = fs::read_link(entry.path()) {
                if !path.starts_with("../../devices/virtual/") {
                    let ifindex = fs::read_to_string(entry.path().join("ifindex")).unwrap();
                    let mac = fs::read_to_string(entry.path().join("address")).unwrap();
                    let oper = fs::read_to_string(entry.path().join("operstate")).unwrap();

                    let interface = Interface {
                        index: ifindex.trim().parse().unwrap(),
                        name: entry.file_name().into_string().unwrap(),
                        state: oper.parse::<OperState>().unwrap(),
                        mac: mac.parse::<MacAddress>().unwrap(),
                    };
                    nics.push(interface);
                }
            }
        }
    }
    nics
}

pub struct Socket {
    fd: AsyncFd<OwnedFd>,
    proto: u16,
}

impl Socket {
    pub fn new(proto: u16) -> io::Result<Self> {
        match unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, proto.to_be().into()) } {
            -1 => Err(io::Error::last_os_error()),
            fd => {
                unsafe {
                    let flag = libc::fcntl(fd, libc::F_GETFL, 0);
                    libc::fcntl(fd, libc::F_SETFL, flag | libc::O_NONBLOCK);
                }
                Ok(Socket {
                    fd: AsyncFd::new(unsafe { OwnedFd::from_raw_fd(fd) })?,
                    proto,
                })
            },
        }
    }

    pub fn bind(&self, ifindex: u32) -> io::Result<()> {
        let sa = libc::sockaddr_ll {
            sll_family: libc::AF_PACKET as libc::sa_family_t,
            sll_protocol: self.proto.to_be(),
            sll_ifindex: ifindex as i32,
            sll_hatype: 0,
            sll_pkttype: 0,
            sll_halen: 0,
            sll_addr: [0; 8]
        };
        let addr_ptr = &raw const sa as *const libc::sockaddr;
        unsafe {
            let addr_len: libc::socklen_t = mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t;
            match libc::bind(self.fd.get_ref().as_raw_fd(), addr_ptr, addr_len) {
                -1 => {
                    let err = io::Error::last_os_error();
                    error!("bind failed: {}, kind: {:?}", err, err.kind());
                    Err(err)
                },
                _ => Ok(()),
            }
        }
    }

    pub fn set_promiscuous(&self, promisc: bool, ifindex: u32) -> io::Result<()> {
        let req = libc::packet_mreq {
            mr_ifindex: ifindex as i32,
            mr_type: libc::PACKET_MR_PROMISC as u16,
            mr_alen: 0,
            mr_address: [0u8; 8],
        };

        if unsafe {
            libc::setsockopt(
                self.fd.get_ref().as_raw_fd(),
                libc::SOL_PACKET,
                if promisc {
                    libc::PACKET_ADD_MEMBERSHIP
                } else {
                    libc::PACKET_DROP_MEMBERSHIP
                },
                &raw const req as *const libc::c_void,
                mem::size_of::<libc::packet_mreq>() as libc::socklen_t,
            ) != 0
        } {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> io::Result<u32> {
        loop {
            let mut guard = self.fd.readable().await?;
            match guard.try_io(|inner| recv(inner.get_ref().as_raw_fd(), buf)) {
                Ok(ifindex) => return ifindex,
                Err(_would_block) => {
                    continue
                },
            }
        }
    }

    pub async fn send(&mut self, buf: &[u8], ifindex: u32) -> io::Result<isize> {
        let sa = libc::sockaddr_ll {
            sll_family: libc::AF_PACKET as libc::sa_family_t,
            sll_protocol: self.proto.to_be(),
            sll_ifindex: ifindex as i32,
            sll_hatype: 0,
            sll_pkttype: 0,
            sll_halen: 0,
            sll_addr: [0; 8]
        };
        loop {
            let mut guard = self.fd.writable().await?;
            match guard.try_io(|inner| send(inner.get_ref().as_raw_fd(), buf, &sa)) {
                Ok(result) => return result,
                Err(_would_block) => continue,
            }
        }
    }
}

fn recv(fd: i32, buf: &mut [u8]) -> io::Result<u32> {                                                                                                                         
    let mut sa: libc::sockaddr_ll = unsafe { mem::zeroed() };
    let mut addr_len = mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t;
    debug!("recv from socket {fd}");
    unsafe {
        match libc::recvfrom(fd,
                       buf.as_mut_ptr() as *mut libc::c_void,
                       buf.len(),
                       0,   // flags
                       &raw mut sa as *mut libc::sockaddr,
                       &mut addr_len) {
                       //std::ptr::null_mut(),
                       //std::ptr::null_mut()) {
            -1 => {
                let err = io::Error::last_os_error(); // io::ErrorKind::WouldBlock
                //error!("recv failed: {}, kind: {:?}", err, err.kind());
                Err(err)
            },
            len => {
                let iface_index = sa.sll_ifindex as u32;
                debug!("fd({fd}) len: {len}, from nic: {iface_index}");
                Ok(iface_index)
            }
        }
    }
}

fn send(fd: i32, buf: &[u8], sa: *const libc::sockaddr_ll) -> io::Result<isize> {
    let addr_ptr = sa as *const libc::sockaddr;
    let addr_len: libc::socklen_t = mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t;
    debug!("send to socket {fd} | {buf:02x?}");
    unsafe {
        debug!("send to ifindex {}, proto {:0x}, family {:0x}", (*sa).sll_ifindex, (*sa).sll_protocol, (*sa).sll_family);
        match libc::sendto(fd,
                     buf.as_ptr() as *const libc::c_void,
                     buf.len(),
                     0,   // flags
                     addr_ptr,
                     addr_len) {
            len if len < 0 => {
                let err = io::Error::last_os_error(); // io::ErrorKind::WouldBlock
                error!("send failed: {}, kind: {:?}", err, err.kind());
                Err(err)
            },
            len => {
                debug!("send fd({fd}) len: {len}, from nic: {}", (*sa).sll_ifindex);
                Ok(len)
            }
        }
    }
}
