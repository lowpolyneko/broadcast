use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Result, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::slice;

use epoll;

const ADDR: &str = "127.0.0.1:6687";

struct ClientSession {
    stream: TcpStream,
    recv_buffer: Vec<u8>,
    send_buffer: Vec<u8>
}

fn handle_client(fd: RawFd, clients: &mut HashMap<RawFd, ClientSession>) -> Result<()> {
    let mut buffer = Vec::new();
    {
        let session = clients.get_mut(&fd).expect("failed to retrieve client stream!");
        session.stream.read(&mut buffer)?;
        session.recv_buffer.append(&mut buffer);
        if session.recv_buffer.len() >= 255 {
            // TODO: put in a send request
            buffer.append(&mut session.recv_buffer);
        }
    }

    if !buffer.is_empty() {
        for c in clients.values_mut() {
            c.send_buffer.extend_from_slice(&buffer);
        }
    }

    Ok(())
}

fn send_payloads(session: &mut ClientSession) -> Result<()> {
    session.stream.write_all(&session.send_buffer)?;
    session.send_buffer.clear();
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

        // check for reads
        if epoll_event.events & epoll::Events::EPOLLIN.bits() != 0 {
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
                           epoll::Event::new(epoll::Events::EPOLLIN.union(epoll::Events::EPOLLOUT), fd as u64)
                           )?;

                client_sockets.insert(socket.as_raw_fd(), ClientSession {
                    stream: socket,
                    send_buffer: Vec::new(),
                    recv_buffer: Vec::new()
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

        // check for writes
        if epoll_event.events & epoll::Events::EPOLLOUT.bits() != 0 {
            let fd = epoll_event.data as RawFd;
            let session = client_sockets.get_mut(&fd).expect("failed to retrieve client stream!");
            if send_payloads(session).is_err() {
                // socket closed
                client_sockets.remove(&fd);
            }
        }
    }
}
