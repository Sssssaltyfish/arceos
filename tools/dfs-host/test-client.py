import socket

# Define the server address and port
server_address = ('127.0.0.1', 8000)

# Create a socket object for a TCP connection
client_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

# Connect to the server
client_socket.connect(server_address)

# Define the message to send
message = "Hello"

# Send the message to the server
client_socket.send(message.encode())

# Close the socket when done
client_socket.close()
