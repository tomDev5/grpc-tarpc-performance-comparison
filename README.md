# Grpc-tarpc frankenstein performance comparison

This repository compares the performance of regular gRPC vs switching the HTTP transport for tarpc's bincode-based format.

This is a very ugly experiment, the connection between tarpc and gRPC is not generic to RPC and cannot support streaming, and I only did it to check if HTTP2 can be a bottleneck in high throughput projects.

On my machine it appears it can indeed:
```
# time ./target/release/tarpc # ran 3 times and averaged
real    0m3.611s

# time ./target/release/regular # ran 3 times and averaged
real    0m9.764s
```