//#![feature(future_join)]

use std::future::Future;
//use std::future::join;
use std::task::{Poll, Context};
use std::pin::Pin;
use std::mem;
use std::io;
use tokio;
use libc::{
    c_void,
    socket,
    recvfrom,
    sockaddr,
    sockaddr_ll,
    socklen_t,
    SOCK_RAW,
    AF_PACKET,
};

struct SocketRead<'a> {
    fd: i32,
    buf: &'a mut [u8],
}

impl Future for SocketRead<'_> {
    type Output = isize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut sender_addr: sockaddr_ll = unsafe { mem::zeroed() };

        let mut addr_buf_sz: socklen_t = mem::size_of::<sockaddr_ll>() as socklen_t;
        //println!("recv from socket {}", self.fd);
        unsafe {
            let addr_ptr = mem::transmute::<*mut sockaddr_ll, *mut sockaddr>(&mut sender_addr);
            match recvfrom(self.fd,
                           self.buf.as_mut_ptr() as *mut c_void,
                           self.buf.len(),
                           0,   // flags
                           addr_ptr,
                           &mut addr_buf_sz) {
                -1 => {
                    let err = io::Error::last_os_error(); // io::ErrorKind::WouldBlock
                    //eprintln!("failed: {}, kind: {:?}", err, err.kind());
                    if err.kind() == io::ErrorKind::WouldBlock {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    } else {
                        Poll::Ready(0)
                    }
                },
                len => {
                    println!("fd:{} len: {len} = {:?}", self.fd, self.buf);
                    Poll::Ready(len)
                }
            }
        }
    }
}

async fn async_recv(fd: i32, buf: &mut [u8]) -> isize {
    let sock = SocketRead {
        fd,
        buf,
    };
    sock.await
}

async fn recv(fd: i32, buf: &mut [u8]) -> isize {
//fn recv(fd: i32, buf: &mut [u8]) -> isize {
    let mut sender_addr: sockaddr_ll = unsafe { mem::zeroed() };

    let len: isize;
    let mut addr_buf_sz: socklen_t = mem::size_of::<sockaddr_ll>() as socklen_t;
    println!("recv from socket {fd}");
    unsafe {
        let addr_ptr = mem::transmute::<*mut sockaddr_ll, *mut sockaddr>(&mut sender_addr);
        len = match recvfrom(fd,
                             buf.as_mut_ptr() as *mut c_void,
                             buf.len(),
                             0,   // flags
                             addr_ptr,
                             &mut addr_buf_sz) {
            -1 => {
                let err = io::Error::last_os_error(); // io::ErrorKind::WouldBlock
                eprintln!("failed: {}, kind: {:?}", err, err.kind());
                0
            },
            len => {
                let iface_index = sender_addr.sll_ifindex;
                //println!("len: {len} = {buf:?}, from nic: {iface_index}");
                println!("len: {len}, from nic: {iface_index}");
                len
            }
        }

    }
    println!("fd({fd}): recv {len} bytes");
    len
}

fn open_sock(proto: u16) -> io::Result<i32> {
    match unsafe { socket(AF_PACKET, SOCK_RAW, proto.to_be().into()) } {
        -1 => Err(io::Error::last_os_error()),
        fd => {
            unsafe {
                let flag = libc::fcntl(fd, libc::F_GETFL, 0);
                //libc::fcntl(fd, libc::F_SETFL, flag | libc::O_NONBLOCK);
                Ok(fd)
            }
        }
    }
}

//#[tokio::main(flavor = "current_thread")]
#[tokio::main]
async fn main() {
    //let a1 = async {
    //    let fd = open_sock(libc::ETH_P_ALL as u16).unwrap();
    //    let mut buf: [u8; 1024] = [0; 1024];
    //    async_recv(fd, &mut buf).await
    //};
    //let a2 = async {
    //    let fd = open_sock(libc::ETH_P_AARP as u16).unwrap();
    //    let mut buf: [u8; 1024] = [0; 1024];
    //    async_recv(fd, &mut buf).await
    //};
    //let a3 = async {
    //    let fd = open_sock(libc::ETH_P_ARP as u16).unwrap();
    //    let mut buf: [u8; 1024] = [0; 1024];
    //    async_recv(fd, &mut buf).await
    //};

    //tokio::pin!(a1);
    //tokio::pin!(a2);
    //tokio::pin!(a3);
    //loop {
    //    tokio::select! {
    //        len = &mut a1 => println!("a1 got {len}"),
    //        len = &mut a2 => println!("a2 got {len}"),
    //        len = &mut a3 => println!("a3 got {len}"),
    //    }
    //}

    //join!(a1, a2, a3).await;

    let mut handlers: Vec<tokio::task::JoinHandle<isize>> = vec![];

    //handlers.push(tokio::spawn(async {
    //    let fd = open_sock(libc::ETH_P_ALL as u16).unwrap();
    //    loop {
    //        let mut buf: [u8; 1024] = [0; 1024];

    //        recv(fd, &mut buf).await;
    //        //async_recv(fd, &mut buf).await
    //    }
    //}));

    //handlers.push(tokio::spawn(async {
    //    let fd = open_sock(libc::ETH_P_AARP as u16).unwrap();
    //    loop {
    //        let mut buf: [u8; 1024] = [0; 1024];

    //        recv(fd, &mut buf).await;
    //        //async_recv(fd, &mut buf).await
    //    }
    //}));

    handlers.push(tokio::spawn(async {
        let fd = open_sock(libc::ETH_P_ARP as u16).unwrap();
        loop {
            let mut buf: [u8; 1024] = [0; 1024];

            recv(fd, &mut buf).await;
            //async_recv(fd, &mut buf).await
        }
    }));

    for handler in handlers {
        handler.await;
    }
}
