use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::slice;

use epoll;

const ADDR: &str = "127.0.0.1:6687";

fn handle_client(mut stream: TcpStream, clients: &Vec<TcpStream>) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    stream.read(&mut buffer)?;

    println!("{:?}", buffer); // debug

    for mut c in clients {
        if c.as_raw_fd() != stream.as_raw_fd() {
            c.write_all(&buffer)?;
        }
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDR)?;
    let epoll_fd = epoll::create(true)?;
    let mut epoll_event = epoll::Event { // we're only handling epoll events one event at a time!
        events: 0,
        data: 0
    };
    let mut client_sockets = Vec::new();

    listener.set_nonblocking(true)?; // non-blocking accept

    epoll::ctl(epoll_fd,
               epoll::ControlOptions::EPOLL_CTL_ADD,
               listener.as_raw_fd(),
               epoll::Event::new(epoll::Events::EPOLLIN, listener.as_raw_fd() as u64)
               )?;

    loop {
        epoll::wait(epoll_fd, -1, slice::from_mut(&mut epoll_event))?;

        // check for connections from the main socket
        if listener.as_raw_fd() == epoll_event.data as i32 {
            let (socket, _addr) = listener.accept()?;
            socket.set_nonblocking(true)?;

            // add new socket to epoll_fd
            epoll::ctl(epoll_fd,
                       epoll::ControlOptions::EPOLL_CTL_ADD,
                       socket.as_raw_fd(),
                       epoll::Event::new(epoll::Events::EPOLLIN, socket.as_raw_fd() as u64)
                       )?;

            client_sockets.push(socket);
            continue;
        }

        // otherwise its a client
        unsafe {
            handle_client(TcpStream::from_raw_fd(epoll_event.data as RawFd), &client_sockets)?;
        }
    }
}
