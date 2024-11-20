# Laqista

## Components

Laqista conists of three primary components.

- Deployment
- Server Daemon
- App Instance
- Scheduler

### Server Daemon

Server Daemon runs every Server.  
It is responsible for proxying incoming request to the approriate App Instance.

It is implemented in Go, using [grpc-proxy](https://github.com/mwitkow/grpc-proxy), to reduce implementation effort.

## Development

### Setup

On macOS, `powermetrics` must be executable by `sudo`, without password.

I.e., add the following to `sudoers` file:

```
your-user-name    ALL= NOPASSWD: /usr/bin/powermetrics
```

### Testing

```
grpcurl -plaintext -import-path ./proto -proto laqista.proto -d '{}' '127.0.0.1:50051' laqista.ServerDaemon/Ping
```

### Notes

- Server IDs are UUIDv6, which is based on MAC address
- Deployment IDs are UUIDv4, which is generated randomly 