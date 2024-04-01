extern crate test;

#[cfg(test)]
mod tests {
    use test::Bencher;
    use tokio::runtime;

    use crate::{
        app::{self, proto::HelloRequest},
        proto::{self, DeployRequest, LookupRequest},
    };

    use super::*;

    #[bench]
    fn bench_scheduled(b: &mut Bencher) {
        let runtime = runtime::Runtime::new().unwrap();
        let addr = "http://127.0.0.1:50051";

        let (client, mut app_client, deploy_response) = runtime.block_on(async {
            let mut client = proto::scheduler_client::SchedulerClient::connect(addr)
                .await
                .expect("failed to connect to the server");

            let app_client = app::proto::greeter_client::GreeterClient::connect(addr)
                .await
                .unwrap();

            let request = DeployRequest {
                source: "https://github.com/kino-ma/MLess".to_owned(),
                authoritative: true,
            };

            let deployment = client.deploy(request).await.expect("failed to deploy");

            (client, app_client, deployment)
        });

        let deployment = deploy_response.into_inner().deployment.unwrap();
        let deployment_id = deployment.id;

        b.iter(|| {
            runtime.block_on(async {
                let request = LookupRequest {
                    deployment_id: deployment_id.clone(),
                    qos: None,
                };

                let _resp = client.clone().lookup(request).await.unwrap().into_inner();

                // let mut app_client = app::proto::greeter_client::GreeterClient::connect(addr)
                //     .await
                //     .unwrap();
                let request = HelloRequest {
                    name: "MLess benchamrk".to_owned(),
                };
                app_client.say_hello(request).await.unwrap();
            })
        })
    }

    #[bench]
    fn bench_direct(b: &mut Bencher) {
        let runtime = runtime::Runtime::new().unwrap();
        let addr = "http://127.0.0.1:50051";

        let mut app_client = runtime.block_on(async {
            let app_client = app::proto::greeter_client::GreeterClient::connect(addr)
                .await
                .unwrap();
            app_client
        });

        b.iter(|| {
            runtime.block_on(async {
                let request = HelloRequest {
                    name: "MLess benchamrk".to_owned(),
                };
                app_client.say_hello(request).await.unwrap();
            })
        })
    }
}
