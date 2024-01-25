use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Result, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::slice;

use epoll;

const ADDR: &str = "127.0.0.1:6687";

struct ClientSession {
    stream: TcpStream,
    buffer: Vec<u8>
}

fn handle_client(fd: RawFd, clients: &mut HashMap<RawFd, ClientSession>) -> Result<()> {
    let mut send_buffer = Vec::new();
    let mut recv_buffer = [0];

    {
        let session = clients.get_mut(&fd).expect("failed to retrieve client stream!");
        session.stream.read_exact(&mut recv_buffer)?;
        session.buffer.push(recv_buffer[0]);
        if session.buffer.len() >= 255 || recv_buffer[0] == b'\0' {
            send_buffer.append(&mut session.buffer);
        }
    }

    if !send_buffer.is_empty() {
        println!("{}", std::str::from_utf8(&send_buffer).expect("client send invalid data!"));
        for c in clients.values_mut() {
            if c.stream.as_raw_fd() != fd {
                c.stream.write_all(&send_buffer)?;
            }
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let listener = TcpListener::bind(ADDR)?;
    let epoll_fd = epoll::create(true)?;
    let mut epoll_event = epoll::Event { // we're only handling epoll events one event at a time!
        events: 0,
        data: 0
    };
    let mut client_sockets = HashMap::new();

    listener.set_nonblocking(true)?; // non-blocking accept

    epoll::ctl(epoll_fd,
               epoll::ControlOptions::EPOLL_CTL_ADD,
               listener.as_raw_fd(),
               epoll::Event::new(epoll::Events::EPOLLIN, listener.as_raw_fd() as u64)
               )?;

    println!("Waiting for connections...");

    loop {
        epoll::wait(epoll_fd, -1, slice::from_mut(&mut epoll_event))?;

        // check for connections from the main socket
        if listener.as_raw_fd() == epoll_event.data as i32 {
            let (socket, _addr) = listener.accept()?;
            socket.set_nonblocking(true)?;

            println!("Connection accepted!");

            // add new socket to epoll_fd
            let fd = socket.as_raw_fd();
            epoll::ctl(epoll_fd,
                       epoll::ControlOptions::EPOLL_CTL_ADD,
                       fd,
                       epoll::Event::new(epoll::Events::EPOLLIN, fd as u64)
                       )?;

            client_sockets.insert(socket.as_raw_fd(), ClientSession {
                stream: socket,
                buffer: Vec::new()
            });

            continue;
        }

        // otherwise its a client
        let fd = epoll_event.data as RawFd;
        if handle_client(fd, &mut client_sockets).is_err() {
            // socket closed
            client_sockets.remove(&fd);
        }
    }
}
