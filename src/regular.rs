use helloworld::{
    greeter_client::GreeterClient,
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest,
};
use tonic::{transport::Server, Request, Response, Status};

mod helloworld;

#[derive(Default)]
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
                let addr = "[::1]:50051".parse().unwrap();
                let greeter = MyGreeter::default();
                tokio::select! {
                    _ = Server::builder()
                    .add_service(GreeterServer::new(greeter))
                    .serve(addr) => panic!("Server ended!"),
                    _ = stop_receiver => (),
                };
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
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let client = GreeterClient::connect("http://[::1]:50051").await.unwrap();
                let tasks: Vec<_> = (0..32)
                    .map(|_| {
                        let mut client_clone = client.clone();
                        tokio::spawn(async move {
                            for _ in 0..10000 {
                                let request = tonic::Request::new(HelloRequest {
                                    name: "Tonic".into(),
                                });

                                let _response = client_clone.say_hello(request).await.unwrap();
                            }
                        })
                    })
                    .collect();

                futures::future::join_all(tasks).await;
            })
    });

    client_handle.join().unwrap();
    stop_sender.send(()).unwrap();
    server_handle.join().unwrap();
}
