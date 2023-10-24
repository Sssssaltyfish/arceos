import socket

# Define the server address and port
server_address = ('127.0.0.1', 8000)

# Create a socket object for a TCP connection
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

s.bind(server_address)
s.listen()

conn, addr = s.accept()

print("connection from", addr)

recved = conn.recv(4096)

print("recved", recved)
