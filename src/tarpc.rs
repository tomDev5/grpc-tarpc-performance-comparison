use std::sync::Arc;

use futures::StreamExt as _;
use helloworld::{greeter_server::Greeter, HelloReply, HelloRequest};
use prost::Message as _;
use tarpc::server::Channel as _;
use tonic::{Request, Response, Status};

mod helloworld;

// Create the gRPC server as usual
// -------------------------------

#[derive(Default, Debug)]
pub struct MyGreeter {}

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let reply = helloworld::HelloReply {
            message: format!("Hello {}!", request.into_inner().name.repeat(500)),
        };
        Ok(Response::new(reply))
    }
}

// Create the tarpc service to send bytes between threads
// ------------------------------------------------------

#[tarpc::service]
pub trait BytesRpc {
    async fn send(data: Vec<u8>) -> Vec<u8>;
}

#[derive(Default, Debug, Clone)]
pub struct BytesServer {
    server: Arc<MyGreeter>,
}

impl BytesRpc for BytesServer {
    async fn send(self, _: ::tarpc::context::Context, data: Vec<u8>) -> Vec<u8> {
        let request = HelloRequest::decode(&*data).unwrap();
        let response = self
            .server
            .say_hello(Request::new(request))
            .await
            .unwrap()
            .into_inner();

        response.encode_to_vec()
    }
}

// start client and server
// -----------------------

fn main() {
    // start server thread
    let (stop_sender, stop_receiver) = tokio::sync::oneshot::channel();
    let server_handle = std::thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                // create gRPC greeter
                let greeter = Arc::new(MyGreeter::default());
                // create tarpc listener
                let server_addr = (std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 8000);
                let mut listener = tarpc::serde_transport::tcp::listen(
                    &server_addr,
                    tarpc::tokio_serde::formats::Bincode::default,
                )
                .await
                .expect("Failed to listen");
                listener.config_mut().max_frame_length(usize::MAX);

                tokio::select! {
                    _ = listener
                    .filter_map(|r| std::future::ready(r.ok()))
                    .map(tarpc::server::BaseChannel::with_defaults)
                    .map(|channel| {
                        let server = BytesServer {
                            server: greeter.clone(),
                        };
                        channel.execute(server.serve()).for_each(|f| async {
                            tokio::spawn(f);
                        })
                    })
                    .buffer_unordered(10)
                    .for_each(|_| async {}) => panic!("Server ended!"),
                    _ = stop_receiver => (),
                }
            })
    });

    // start client thread
    let client_handle = std::thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                // wait for server to start
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                // create tarpc client
                let transport = tarpc::serde_transport::tcp::connect(
                    (std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 8000),
                    tarpc::tokio_serde::formats::Bincode::default,
                )
                .await
                .expect("Failed to create transport");

                let client =
                    BytesRpcClient::new(tarpc::client::Config::default(), transport).spawn();

                // start tasks
                let tasks: Vec<_> = (0..32)
                    .map(|_| async {
                        for _ in 0..10000 {
                            let request = HelloRequest {
                                name: "Tonic".into(),
                            };

                            let _response = HelloReply::decode(
                                client
                                    .send(tarpc::context::current(), request.encode_to_vec())
                                    .await
                                    .unwrap()
                                    .as_slice(),
                            )
                            .unwrap();
                        }
                    })
                    .collect();

                futures::future::join_all(tasks).await;
            })
    });

    client_handle.join().unwrap();
    stop_sender.send(()).unwrap();
    server_handle.join().unwrap();
}
